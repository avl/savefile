#![allow(warnings)]
use std::collections::HashMap;
use std::io::{BufReader, BufWriter, Read, Write};
use std::marker::PhantomData;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::ptr::slice_from_raw_parts;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::{slice, thread};
use std::thread::JoinHandle;
use std::time::Duration;
use byteorder::{ReadBytesExt, WriteBytesExt};
use crossbeam_channel::{bounded, Receiver, RecvError, Sender};
use parking_lot::Mutex;

use savefile::{AbiTraitDefinition, Deserialize, Deserializer, SavefileError, Serialize, Serializer};
use savefile_abi::{AbiConnection, AbiConnectionTemplate, AbiErrorMsg, AbiExportable, AbiExportableImplementation, AbiProtocol, definition_receiver, EntryKey, EntryPoint, Owning, RawAbiCallResult, TraitObject};


#[repr(u8)]
enum RemoteCommands {
    CallInstanceMethod = 0,
    InterrogateVersion = 1,
    InterrogateMethods = 2,
    CreateInstance = 3,
    DropInstance = 4,
}





fn serve_connection(mut stream: TcpStream, types: HashMap<TraitName, Arc<DynAbiExportableObjectType>>, initial_cmd: u8) -> Result<(), SavefileError>
{
    let mut context = ConnectionContext {
        active_objects: HashMap::new(),
        supported_types: types,
    };
    let mut first_cmd = Some(initial_cmd);
    let mut stream2 = BufWriter::new(stream.try_clone()?);
    let mut ser = Serializer {
        file_version: 0,
        writer: &mut stream2,
    };
    let mut stream = BufReader::new(stream);
    let mut deser = Deserializer {
        reader: &mut stream,
        file_version: 0,
        ephemeral_state: Default::default(),
    };
    loop {
        let cmd = if let Some(x) = first_cmd.take() {x} else {deser.read_u8()?};
        let mut buf = vec![];
        match cmd {
            6 => {
                return Ok(());
            }
            4/*RemoteCommands::CreateInstance*/ => {
                let name = TraitName(deser.read_string()?);
                let objtype = context.supported_types.get(&name).ok_or_else(||SavefileError::GeneralError {msg:format!("Unsupported trait '{}'", name.0)})?;
                let mut trait_object = TraitObject::zero();
                let mut error_receiver = String::new();
                unsafe extern "C" fn error_callback_fn(error_receiver: *mut (), error: *const AbiErrorMsg) {
                    let error_msg = unsafe { &mut *(error_receiver as *mut String) };
                    *error_msg = unsafe { &*error }.convert_to_string();
                }
                unsafe { (objtype.local_entry)(AbiProtocol::CreateInstance {
                    trait_object_receiver: &mut trait_object,
                    error_receiver: &mut error_receiver as *mut String as *mut _,
                    error_callback: error_callback_fn,
                }); }
                if error_receiver.is_empty() == false {
                    ser.write_u8(0)?;
                    ser.write_string(&error_receiver)?;
                } else {
                    ser.write_u8(1)?;
                    trait_object.as_usize_tuples().serialize(&mut ser)?;
                    context.active_objects.insert(TraitKey(trait_object.as_usize_tuples()), DynAbiExportableObject {
                        object_type: Arc::clone(objtype),
                        trait_object,
                    });
                }
                ser.writer.flush()?;
            }
            5 /*RemoteCommands::DropInstance*/ => {
                let trait_object_key = TraitKey(<_ as Deserialize>::deserialize(&mut deser)?);
                let obj = context.active_objects.get(&trait_object_key).ok_or_else(||SavefileError::GeneralError {msg:format!("Unknown object[5]: {:?}", trait_object_key)})?;
                unsafe { (obj.object_type.local_entry)(AbiProtocol::DropInstance {
                    trait_object: obj.trait_object,
                }) }
                String::default().serialize(&mut ser)?;
                ser.writer.flush()?;
            }
            3 /*RemoteCommands::InterrogateMethods*/ => {
                let name = TraitName(deser.read_string()?);
                let objtype = context.supported_types.get(&name).ok_or_else(||SavefileError::GeneralError {msg:format!("Unsupported trait '{}'", name.0)})?;

                let schema_version_required = deser.read_u16()?;
                let callee_schema_version_interrogated = deser.read_u32()?;


                unsafe extern "C" fn raw_data_definition_receiver(receiver: *mut (), callee_schema_version: u16, data: *const u8, data_len: usize) {
                    let ser = receiver as *mut Serializer<'_, BufWriter<TcpStream>>;
                    let mut ser = unsafe { &mut *ser};
                    _ = ser.write_u16(callee_schema_version);
                    _ = ser.write_usize(data_len);
                    _ = ser.write_bytes(unsafe { slice::from_raw_parts(data,  data_len) } );
                }

                unsafe {
                    (objtype.local_entry)(AbiProtocol::InterrogateMethods {
                        schema_version_required,
                        callee_schema_version_interrogated,
                        result_receiver: &mut ser as *mut _ as *mut _,
                        callback: raw_data_definition_receiver,
                    })
                }
                ser.writer.flush()?;

            }
            1 /*RemoteCommands::InterrogateVersion*/ => {
                let key = TraitName::deserialize(&mut deser)?;
                let obj  = context.supported_types.get(&key).ok_or_else(||SavefileError::GeneralError {msg:format!("Unknown object[1] '{:?}'", key)})?;

                let mut schema_version_receiver: u16 = 0;
                let mut abi_version_receiver: u32 = 0;
                unsafe { (obj.local_entry)(AbiProtocol::InterrogateVersion {
                    schema_version_receiver: &mut schema_version_receiver as *mut _,
                    abi_version_receiver: &mut abi_version_receiver as *mut _,
                }); }
                ser.write_u16(schema_version_receiver)?;
                ser.write_u32(abi_version_receiver)?;
                ser.writer.flush()?;
            }
            0/*RemoteCommands::CallInstanceMethod*/ => {
                let trait_object_key = TraitKey(<_ as Deserialize>::deserialize(&mut deser)?);
                let method_number = deser.read_u16()?;

                let obj = context.active_objects.get(&trait_object_key).ok_or_else(||SavefileError::GeneralError {msg:format!("Unknown object[0]: {:?}", trait_object_key)})?;
                let datasize = deser.read_usize()?;
                if buf.len() < datasize {
                    buf.resize(datasize, 0);
                }
                let mut result = DynAbiCallResult::AbiError(String::new());
                deser.read_bytes_to_buf(&mut buf[0..datasize])?;

                unsafe extern "C" fn do_receive(
                    outcome: *const RawAbiCallResult,
                    result_receiver: *mut (), /*Result<T,SaveFileError>>*/
                ) {
                    let result_receiver = unsafe { &mut *(result_receiver as *mut DynAbiCallResult) };
                    match unsafe { &*outcome } {
                        RawAbiCallResult::Success { data, len } => {
                            *result_receiver = DynAbiCallResult::Success( unsafe { (*slice_from_raw_parts(*data, *len)).into() });
                        }
                        RawAbiCallResult::Panic(msg) => {
                            *result_receiver = DynAbiCallResult::Panic(msg.convert_to_string())
                        }
                        RawAbiCallResult::AbiError(msg) => {
                            *result_receiver = DynAbiCallResult::AbiError(msg.convert_to_string())
                        }
                    }
                }

                unsafe { (obj.object_type.local_entry)(
                    AbiProtocol::RegularCall {
                        trait_object: obj.trait_object,
                        compatibility_mask: 0,
                        data: buf.as_ptr(),
                        data_length: datasize,
                        abi_result: &mut result as *mut DynAbiCallResult as *mut _,
                        receiver: do_receive,
                        effective_version: 0, //TODO: Fix, shouldn't be hard-coded 0 here!
                        method_number,
                    }
                ); }

                println!("Sending back result: {:?}", &result);
                result.serialize(&mut ser)?;
                ser.writer.flush()?;
            }
            _ => {
                println!("Unsupported command: {}", cmd);
                return Err(SavefileError::GeneralError {msg: format!("Unsupported command: {}", cmd)});
            }
        }
    }
}

enum BackgroundListenerResult {
    QuitNormally,
    FailedToBindSocket,
}
#[derive(Savefile,Debug)]
pub enum DynAbiCallResult {
    Success(Box<[u8]>),
    Panic(String),
    AbiError(String),
}

pub unsafe trait DynAbiExportable {
    /// Get the name of the exported trait
    fn get_name(&self) -> String;
    /// A function which implements the savefile-abi contract.
    fn get_abi_entry(&self) -> unsafe extern "C" fn(AbiProtocol);
    /// Must return a truthful description about all the methods in the
    /// `dyn trait` that AbiExportable is implemented for (i.e, `Self`).
    fn get_definition(&self, version: u32) -> AbiTraitDefinition;
    /// Must return the current latest version of the interface. I.e,
    /// the version which Self represents. Of course, there may be future higher versions,
    /// but none such are known by the code.
    fn get_latest_version(&self) -> u32;
    /// Implement method calling. Must deserialize data from 'data', and
    /// must return an outcome (result) by calling `receiver`.
    ///
    /// The return value is either Ok, or an error if the method to be called could
    /// not be found or for some reason not called (mismatched actual ABI, for example).
    ///
    /// `receiver` must be given 'abi_result' as its 'result_receiver' parameter, so that
    /// the receiver may set the result. The receiver executes at the caller-side of the ABI-divide,
    /// but it receives as first argument an RawAbiCallResult that has been created by the callee.
    fn call(
        &mut self,
        tcp_stream: &TcpStream,
        method_number: u16,
        effective_version: u32,
        data: &[u8],
    ) -> Result<DynAbiCallResult, SavefileError>;
}

struct DynAbiExportableObject {
    object_type: Arc<DynAbiExportableObjectType>,
    trait_object: TraitObject,
}

#[derive(Savefile,Debug,Clone,PartialEq,Eq,PartialOrd,Ord,Hash)]
struct TraitName(String);
#[derive(Savefile,Debug,Clone,PartialEq,Eq,PartialOrd,Ord,Hash)]
struct TraitKey((usize,usize));

struct DynAbiExportableObjectType {
    name: TraitName,
    local_entry: unsafe extern "C" fn(AbiProtocol),
    definitions: fn(version:u32) -> AbiTraitDefinition,
    latest_version: u32,
}

struct ConnectionContext {
    active_objects: HashMap<TraitKey, DynAbiExportableObject>,
    supported_types: HashMap<TraitName, Arc<DynAbiExportableObjectType>>
}


struct ClientCommand {
    input: AbiProtocol,
}

struct ClientConnectionState {
    ser: BufWriter<TcpStream>,
    deser: BufReader<TcpStream>,
}

impl Drop for ClientConnectionState {
    fn drop(&mut self) {
        _ = self.ser.write_u8(6);
        _ = self.ser.flush();
    }
}

struct ClientConnection {
    key: EntryKey,
    trait_name: TraitName,
    conn: Arc<Mutex<ClientConnectionState>>,
}



static CONNECTION_ID: AtomicU64 = AtomicU64::new(0);
fn process_client_command<W:Write, R: Read>(ser: &mut Serializer<W>, deser: &mut Deserializer<R>,  cmd: ClientCommand, trait_name: &TraitName) -> Result<(), SavefileError> {

    match cmd.input {
        AbiProtocol::RegularCall { trait_object, compatibility_mask, data, data_length, abi_result, receiver, effective_version, method_number } => {
            //TODO: Use 'effective_version' !!
            //let key = trait_object.as_usize_tuples();
            ser.write_u8(0)?; // RegularCall
            let traitkey = TraitKey(trait_object.as_usize_tuples());
            traitkey.serialize(ser)?;
            ser.write_u16(method_number)?;
            /*ser.write_usize(key.0)?;
            ser.write_usize(key.1)?;
            ser.write_u32(effective_version)?;*/
            let argdata = unsafe { std::slice::from_raw_parts(data, data_length) };
            ser.write_usize(argdata.len())?;
            ser.write_bytes(argdata)?;
            ser.writer.flush()?;

            let dynresult  = DynAbiCallResult::deserialize(deser)?;
            match dynresult {
                DynAbiCallResult::Success(s) => {
                    unsafe { receiver(&RawAbiCallResult::Success {
                        data: s.as_ptr(),
                        len: s.len(),
                    }, abi_result) }
                }
                DynAbiCallResult::Panic(p) => {
                    unsafe { receiver(&RawAbiCallResult::AbiError(AbiErrorMsg::from(&p)), abi_result) }
                }
                DynAbiCallResult::AbiError(e) => {
                    unsafe { receiver(&RawAbiCallResult::AbiError(AbiErrorMsg::from(&e)), abi_result) }
                }
            }

            Ok(())
        }
        AbiProtocol::InterrogateVersion { schema_version_receiver, abi_version_receiver } => {
            ser.write_u8(1)?; // RemoteCommands::InterrogateVersion
            trait_name.serialize(ser)?;
            ser.writer.flush()?;
            unsafe {
                *schema_version_receiver = deser.read_u16()?;
                *abi_version_receiver = deser.read_u32()?;
            }
            Ok(())
        }
        AbiProtocol::InterrogateMethods { schema_version_required, callee_schema_version_interrogated, result_receiver, callback } => {
            ser.write_u8(3)?; //InterrogateMethods
            trait_name.serialize(ser)?;
            ser.write_u16(schema_version_required)?;
            ser.write_u32(callee_schema_version_interrogated)?;
            ser.writer.flush()?;
            let callee_schema_version = deser.read_u16()?;
            let response_len = deser.read_usize()?;
            let response = deser.read_bytes(response_len)?; //TODO: Optimize, this always allocates!
            unsafe {
                callback(result_receiver, callee_schema_version, response.as_ptr(), response.len());
            }
            Ok(())
        }
        AbiProtocol::CreateInstance { trait_object_receiver, error_receiver, error_callback } => {
            ser.write_u8(4)?; //CreateInstance
            ser.write_string(&trait_name.0)?;
            ser.writer.flush()?;
            match deser.read_u8()? {
                1 => {
                     // Success
                    let to = unsafe { TraitObject::from_usize_without_provenance(deser.read_usize()?, deser.read_usize()?) };
                    unsafe {
                        *trait_object_receiver = to;
                    }
                }
                _ => {
                    let response = deser.read_string()?;
                    unsafe { error_callback(error_receiver, &AbiErrorMsg {
                        len: response.len(),
                        error_msg_utf8: response.as_ptr(),
                    }) }
                }
            }
            Ok(())
        }
        AbiProtocol::DropInstance { trait_object } => {
            ser.write_u8(5)?; //DropInstance
            let to = trait_object.as_usize_tuples();
            ser.write_usize(to.0)?;
            ser.write_usize(to.1)?;
            ser.writer.flush()?;
            let err = deser.read_string()?;
            if !err.is_empty() {
                Err(SavefileError::GeneralError {
                    msg: err,
                })
            } else {
                Ok(())
            }
        }
    }
}


impl ClientConnection {

    /// TODO: Optimize - add constructor that doesn't create new conn every time!
    pub fn new(addr: impl ToSocketAddrs, trait_name: &str) -> Result<ClientConnection,SavefileError> {
        let mut stream = TcpStream::connect(addr)?;


        let mut ser = BufWriter::new(stream.try_clone().unwrap());
        let mut deser = BufReader::new(stream);


        Ok(ClientConnection {
            key: EntryKey {
                data1: 1<<63,
                data2: CONNECTION_ID.fetch_add(1, Ordering::Relaxed)
            },
            trait_name: TraitName(trait_name.into()),
            conn: Arc::new(Mutex::new(ClientConnectionState{
                ser,
                deser,
            })),
        })
    }
}

unsafe impl EntryPoint for ClientConnection {
    unsafe fn call(&self, data: AbiProtocol) {
        let mut conn = self.conn.lock();
        let mut conn = &mut *conn;
        let mut ser = Serializer {
            writer: &mut conn.ser,
            file_version: 0,
        };
        let mut deser = Deserializer {
            file_version: 0,
            reader: &mut conn.deser,
            ephemeral_state: Default::default()
        };
        process_client_command(&mut ser, &mut deser, ClientCommand{
            input: data,
        }, &self.trait_name).expect("Remote call failed");
    }

    fn get_key(&self) -> EntryKey {
        self.key
    }
}

struct RemoteEntrypoint {

}

struct Server {
    addr: SocketAddr, //TODO: Replace this janky solution with a mechanism that implements select([accept_fd, local_pipe]) instead.
    jh: Option<JoinHandle<BackgroundListenerResult>>,
    has_quit: bool,
}

impl Server {
    pub fn serve_forever(&mut self) {
        if let Some(x) = self.jh.take() {
            _ = x.join();
        }
        self.has_quit = true;
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if !self.has_quit {
            let target;
            if self.addr.ip().is_unspecified() {
                if self.addr.is_ipv4() {
                    target = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127,0,0,0)),self.addr.port());
                } else {
                    target = SocketAddr::new( "::1".parse().unwrap(),self.addr.port());
                }
            } else {
                target = self.addr;
            }
            if let Ok(mut stream) = TcpStream::connect(target) {
                _ = stream.write_u8(6);
            }
        }
        _ = self.jh.take().map(|x|x.join());
    }
}
fn serve(local: impl ToSocketAddrs + Send + 'static, supported_types: HashMap<TraitName, Arc<DynAbiExportableObjectType>>) -> Result<Server,SavefileError> {
    let listener = match TcpListener::bind(&local) {
        Ok(listener) => listener,
        Err(err) => {
            println!("Failed to bind: {:?}", err);
            return Err(SavefileError::GeneralError {msg: format!("Failed to bind address: {:?}", err)});
        }
    };
    let addr = listener.local_addr()?;
    let jh = thread::spawn(move||{
        'rebind: loop {
            println!("listening started, ready to accept");
            for stream in listener.incoming() {
                let mut stream = match stream {
                    Ok(stream) => stream,
                    Err(err) => {
                        println!("Failed to accept incoming conneciton: {:?}", err);
                        thread::sleep(Duration::from_millis(10));
                        continue 'rebind;
                    }
                };
                match stream.read_u8(){
                    Ok(val) => {
                        if val == 6 {
                            //Time to quit
                            println!("Received order to quit");
                            return BackgroundListenerResult::QuitNormally;
                        } else {
                            let types = supported_types.clone();
                            thread::spawn(move|| {
                                match serve_connection(stream, types, val) {
                                    Ok(_) => {},
                                    Err(err) => {
                                        println!("Worker error: {:?}", err);
                                    }
                                }
                            });
                        }
                    }
                    Err(err) => {
                        println!("Read stream failed: {:?}", err);
                    }
                }
            }
        }
    });
    Ok(Server {
        addr,
        jh:Some(jh),
        has_quit: false
    })
}

#[savefile_abi_exportable(version=0)]
pub trait TestTrait {
    fn call_trait(&self);
}
#[derive(Default)]
struct TestTraitImpl;
impl TestTrait for TestTraitImpl {
    fn call_trait(&self) {
    }
}

savefile_abi_export!(TestTraitImpl, TestTrait);

#[test]
fn test_server() {
    let mut m = HashMap::new();
    m.insert(TraitName("TestTrait".into()), Arc::new(DynAbiExportableObjectType{
        name: TraitName("TestTrait".into()),
        local_entry: TestTraitImpl::ABI_ENTRY,
        definitions: |ver|<dyn TestTrait as AbiExportable>::get_definition(ver),
        latest_version: 0,
    }));

    let jh = serve("127.0.0.1:1234", m).unwrap();


    let conn = ClientConnection::new("127.0.0.1:1234", "TestTrait").unwrap();

    let abi_conn = AbiConnection::<dyn TestTrait, ClientConnection>::from_entrypoint(conn, None, Owning::Owned).unwrap();

    abi_conn.call_trait();



}