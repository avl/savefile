#![allow(warnings)]
use std::collections::HashMap;
use std::io::{BufReader, BufWriter, Read, Write};
use std::marker::PhantomData;
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::ptr::slice_from_raw_parts;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use byteorder::ReadBytesExt;
use crossbeam_channel::{bounded, Receiver, RecvError, Sender};

use savefile::{AbiTraitDefinition, Deserialize, Deserializer, SavefileError, Serialize, Serializer};
use savefile_abi::{AbiConnectionTemplate, AbiErrorMsg, AbiExportable, AbiProtocol, definition_receiver, EntryKey, EntryPoint, Owning, RawAbiCallResult, TraitObject};


#[repr(u8)]
enum RemoteCommands {
    CallInstanceMethod = 0,
    InterrogateVersion = 1,
    InterrogateMethods = 2,
    CreateInstance = 3,
    DropInstance = 4,
}





fn serve_connection(mut stream: TcpStream, types: HashMap<TraitName, Arc<DynAbiExportableObjectType>>) -> Result<(), SavefileError>
{
    let mut context = ConnectionContext {
        active_objects: HashMap::new(),
        supported_types: types,
    };
    loop {
        let mut stream2 = stream.try_clone()?;
        let mut ser = Serializer {
            file_version: 0,
            writer: &mut stream2,
        };
        let mut deser = Deserializer {
            reader: &mut stream,
            file_version: 0,
            ephemeral_state: Default::default(),
        };
        let cmd = deser.read_u8()?;
        let mut buf = vec![];
        match cmd {
            3/*RemoteCommands::CreateInstance*/ => {
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
                }

            }
            4 /*drop*/ => {
                let trait_object_key = TraitKey(<_ as Deserialize>::deserialize(&mut deser)?);
                let obj = context.active_objects.get(&trait_object_key).ok_or_else(||SavefileError::GeneralError {msg:format!("Unknown object: {:?}", trait_object_key)})?;
                unsafe { (obj.object_type.local_entry)(AbiProtocol::DropInstance {
                    trait_object: obj.trait_object,
                }) }
            }
            2 /*RemoteCommands::InterrogateMethods*/ => {
                let name = TraitName(deser.read_string()?);
                let objtype = context.supported_types.get(&name).ok_or_else(||SavefileError::GeneralError {msg:format!("Unsupported trait '{}'", name.0)})?;

                let schema_version_required = deser.read_u16()?;
                let callee_schema_version_interrogated = deser.read_u32()?;

                let mut result = AbiTraitDefinition {
                    name: "".to_string(),
                    methods: vec![],
                };

                unsafe {
                    (objtype.local_entry)(AbiProtocol::InterrogateMethods {
                        schema_version_required,
                        callee_schema_version_interrogated,
                        result_receiver: &mut result as *mut _,
                        callback: definition_receiver,
                    })
                }
            }
            1 /*RemoteCommands::InterrogateVersion*/ => {
                let key = TraitKey(<(usize,usize) as Deserialize>::deserialize(&mut deser)?);
                let obj  = context.active_objects.get(&key).ok_or_else(||SavefileError::GeneralError {msg:format!("Unknown object '{:?}'", key)})?;

                let mut schema_version_receiver: u16 = 0;
                let mut abi_version_receiver: u32 = 0;
                unsafe { (obj.object_type.local_entry)(AbiProtocol::InterrogateVersion {
                    schema_version_receiver: &mut schema_version_receiver as *mut _,
                    abi_version_receiver: &mut abi_version_receiver as *mut _,
                }); }
                ser.write_u16(schema_version_receiver)?;
                ser.write_u32(abi_version_receiver)?;
            }
            0/*RemoteCommands::CallInstanceMethod*/ => {
                let trait_object_key = TraitKey(<_ as Deserialize>::deserialize(&mut deser)?);
                let method_number = deser.read_u16()?;
                let obj = context.active_objects.get(&trait_object_key).ok_or_else(||SavefileError::GeneralError {msg:format!("Unknown object: {:?}", trait_object_key)})?;
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

                result.serialize(&mut ser)?;
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
#[derive(Savefile)]
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

#[derive(Debug,Clone,PartialEq,Eq,PartialOrd,Ord,Hash)]
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


struct ClientConnection {
    key: EntryKey,
    ser: BufWriter<TcpStream>,
    deser: BufReader<TcpStream>,
}

static CONNECTION_ID: AtomicU64 = AtomicU64::new(0);
fn process_client_command<W:Write, R: Read>(ser: &mut Serializer<W>, deser: &mut Deserializer<R>,  cmd: ClientCommand) -> Result<(), SavefileError> {

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

            let dynresult  = DynAbiCallResult::deserialize(deser)?;
            match dynresult {
                DynAbiCallResult::Success(s) => {
                    unsafe { receiver(&RawAbiCallResult::Success {
                        data: s.as_ptr(),
                        len: s.len(),
                    }, abi_result) }
                }
                DynAbiCallResult::Panic(p) => {
                    receiver(&RawAbiCallResult::AbiError(AbiErrorMsg::from(&p)), abi_result)
                }
                DynAbiCallResult::AbiError(e) => {
                    receiver(&RawAbiCallResult::AbiError(AbiErrorMsg::from(&e)), abi_result)
                }
            }

            Ok(())
        }
        AbiProtocol::InterrogateVersion { schema_version_receiver, abi_version_receiver } => {
            ser.write_u8(1)?; // RemoteCommands::InterrogateVersion
            unsafe {
                *schema_version_receiver = deser.read_u16()?;
                *abi_version_receiver = deser.read_u32()?;
            }
            Ok(())
        }
        AbiProtocol::InterrogateMethods { schema_version_required, callee_schema_version_interrogated, result_receiver, callback } => {
            ser.write_u8(3)?; //InterrogateMethods
            ser.write_u16(schema_version_required)?;
            ser.write_u32(callee_schema_version_interrogated)?;
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
            match deser.read_u8()? {
                0 => {
                     // Success
                    let to = unsafe { TraitObject::from_usize_without_provenance(deser.read_usize()?, deser.read_usize()?) };
                    unsafe {
                        *trait_object_receiver = to;
                    }
                }
                _ => {
                    let response_len = deser.read_usize()?;
                    let response = deser.read_bytes(response_len)?;
                    unsafe { error_callback(error_receiver, &AbiErrorMsg {
                        error_msg_utf8: response.as_ptr(),
                        len: response_len,
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

    pub fn new(addr: impl ToSocketAddrs) -> Result<ClientConnection,SavefileError> {
        let mut stream = TcpStream::connect(addr)?;


        let mut stream2 = BufWriter::new(stream.try_clone().unwrap());
        let mut stream = BufReader::new(stream);
        let mut ser = Serializer {
            writer: &mut stream2,
            file_version: 0,
        };
        let mut deser = Deserializer {
            file_version: 0,
            reader: &mut stream,
            ephemeral_state: Default::default()
        };

        Ok(ClientConnection {
            key: EntryKey {
                data1: 1<<63,
                data2: CONNECTION_ID.fetch_add(1, Ordering::Relaxed)
            },
            ser,
            deser,
        })
    }
}

unsafe impl EntryPoint for ClientConnection {
    unsafe fn call(&self, data: AbiProtocol) -> Result<(), SavefileError> {
        let (retval_sender, retval_receiver) = bounded(1);

        process_client_command(&mut self.ser, &mut self.deser, ClientCommand{
            input: data,

        })
    }

    fn get_key(&self) -> EntryKey {
        self.key
    }
}

struct RemoteEntrypoint {

}

fn serve(local: impl ToSocketAddrs + Send + 'static, supported_types: HashMap<TraitName, Arc<DynAbiExportableObjectType>>) -> JoinHandle<BackgroundListenerResult> {
    thread::spawn(move||{
        'rebind: loop {
            let listener = match TcpListener::bind(&local) {
                Ok(listener) => listener,
                Err(err) => {
                    println!("Failed to bind: {:?}", err);
                    return BackgroundListenerResult::FailedToBindSocket;
                }
            };
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
                        if val == 0 {
                            //Time to quit
                            return BackgroundListenerResult::QuitNormally;
                        } else {
                            let types = supported_types.clone();
                            thread::spawn(|| {
                                match serve_connection(stream, types) {
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
    })
}

#[savefile_abi_exportable(version=0)]
pub trait TestTrait {
    fn call(&self);
}

#[test]
fn test_server() {
    let mut m = HashMap::new();
    m.insert(TraitName("TestTrait".into()), Arc::new(DynAbiExportableObjectType{
        name: TraitName("TestTrait".into()),
        local_entry: <dyn TestTrait as AbiExportable>::ABI_ENTRY,
        definitions: |ver|<dyn TestTrait as AbiExportable>::get_definition(ver),
        latest_version: 0,
    }));
    serve("127.0.0.1:1234", HashMap::new());

}