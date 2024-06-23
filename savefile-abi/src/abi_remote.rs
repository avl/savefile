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
use savefile_derive::Savefile;
use crate::{AbiConnection, AbiConnectionTemplate, AbiErrorMsg, AbiExportable, AbiExportableImplementation, AbiProtocol, definition_receiver, EntryKey, EntryPoint, Owning, RawAbiCallResult, TraitObject};
use crate::serialize_helpers::{SeparateSerializerAndDeserializer, SerializerAndDeserializer};


#[repr(u8)]
enum RemoteCommands {
    CallInstanceMethod = 0,
    InterrogateVersion = 1,
    InterrogateMethods = 2,
    CreateInstance = 3,
    DropInstance = 4,
}



fn serve_connection<S:SerializerAndDeserializer>(mut serder: S, types: HashMap<TraitName, Arc<DynAbiExportableObjectType>>) -> Result<(), SavefileError>
{
    let mut context = ConnectionContext {
        active_objects: HashMap::new(),
        supported_types: types,
    };

    loop {
        let mut deser = serder.get_deserializer();
        let cmd = deser.read_u8()?;
        let mut buf = vec![];
        match cmd {
            6 => {
                //Unexpected, code 6 is for stopping the listener, not the workers!
                return Ok(());
            }
            7 => {
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
                let mut ser = serder.get_serializer();
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
                let mut ser = serder.get_serializer();
                String::default().serialize(&mut ser)?;
                ser.writer.flush()?;
            }
            3 /*RemoteCommands::InterrogateMethods*/ => {
                let name = TraitName(deser.read_string()?);
                let objtype = context.supported_types.get(&name).ok_or_else(||SavefileError::GeneralError {msg:format!("Unsupported trait '{}'", name.0)})?;

                let schema_version_required = deser.read_u16()?;
                let callee_schema_version_interrogated = deser.read_u32()?;


                unsafe extern "C" fn raw_data_definition_receiver<S:SerializerAndDeserializer>(receiver: *mut (), callee_schema_version: u16, data: *const u8, data_len: usize) {
                    let ser = receiver as *mut Serializer<S::TW>;
                    let mut ser = unsafe { &mut *ser};
                    _ = ser.write_u16(callee_schema_version);
                    _ = ser.write_usize(data_len);
                    _ = ser.write_bytes(unsafe { slice::from_raw_parts(data,  data_len) } );
                }

                let mut ser = serder.get_serializer();
                unsafe {
                    (objtype.local_entry)(AbiProtocol::InterrogateMethods {
                        schema_version_required,
                        callee_schema_version_interrogated,
                        result_receiver: &mut ser as *mut _ as *mut _,
                        callback: raw_data_definition_receiver::<S>,
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
                let mut ser = serder.get_serializer();
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
                let mut ser = serder.get_serializer();
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

/// Gives information about the exit of the server background thread
pub enum BackgroundListenerResult {
    /// The server quit normally. This happens if the server
    /// is instructed to quit by one of the clients.
    QuitNormally,
    /// The server failed to bind to the given address
    FailedToBindSocket,
    /// An unexpected problem has caused the server to fail
    UnexpectedError,
    /// 'serve_forever' was called when the server had already stopped.
    AlreadyStopped
}


#[derive(Savefile,Debug)]
enum DynAbiCallResult {
    Success(Box<[u8]>),
    Panic(String),
    AbiError(String),
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

struct ClientConnectionState<T:SerializerAndDeserializer> {
    serder: T
}
impl<T:SerializerAndDeserializer> Drop for ClientConnectionState<T> {
    fn drop(&mut self) {
        _ = self.serder.raw_writer().write_u8(7);
        _ = self.serder.raw_writer().flush();
        self.serder.notify_close();
    }
}

///
pub struct ClientConnection<T:ConnectionStrategy> {
    key: EntryKey,
    trait_name: TraitName,
    conn: Arc<Mutex<ClientConnectionState<T::TS>>>,
}



static CONNECTION_ID: AtomicU64 = AtomicU64::new(0);
fn process_client_command(serder: &mut impl SerializerAndDeserializer, cmd: ClientCommand, trait_name: &TraitName) -> Result<(), SavefileError> {

    match cmd.input {
        AbiProtocol::RegularCall { trait_object, compatibility_mask:_, data, data_length, abi_result, receiver, effective_version, method_number } => {
            //TODO: Use 'effective_version' !!
            //let key = trait_object.as_usize_tuples();
            let mut ser = serder.get_serializer();
            ser.write_u8(0)?; // RegularCall
            let traitkey = TraitKey(trait_object.as_usize_tuples());
            traitkey.serialize(&mut ser)?;
            ser.write_u16(method_number)?;
            /*ser.write_usize(key.0)?;
            ser.write_usize(key.1)?;
            ser.write_u32(effective_version)?;*/
            let argdata = unsafe { std::slice::from_raw_parts(data, data_length) };
            ser.write_usize(argdata.len())?;
            ser.write_bytes(argdata)?;
            ser.writer.flush()?;

            let mut deser = serder.get_deserializer();
            let dynresult  = DynAbiCallResult::deserialize(&mut deser)?;
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
            let mut ser = serder.get_serializer();
            ser.write_u8(1)?; // RemoteCommands::InterrogateVersion
            trait_name.serialize(&mut ser)?;
            ser.writer.flush()?;
            let mut deser = serder.get_deserializer();
            unsafe {
                *schema_version_receiver = deser.read_u16()?;
                *abi_version_receiver = deser.read_u32()?;
            }
            Ok(())
        }
        AbiProtocol::InterrogateMethods { schema_version_required, callee_schema_version_interrogated, result_receiver, callback } => {
            let mut ser = serder.get_serializer();
            ser.write_u8(3)?; //InterrogateMethods
            trait_name.serialize(&mut ser)?;
            ser.write_u16(schema_version_required)?;
            ser.write_u32(callee_schema_version_interrogated)?;
            ser.writer.flush()?;
            let mut deser = serder.get_deserializer();
            let callee_schema_version = deser.read_u16()?;
            let response_len = deser.read_usize()?;
            let response = deser.read_bytes(response_len)?; //TODO: Optimize, this always allocates!
            unsafe {
                callback(result_receiver, callee_schema_version, response.as_ptr(), response.len());
            }
            Ok(())
        }
        AbiProtocol::CreateInstance { trait_object_receiver, error_receiver, error_callback } => {
            let mut ser = serder.get_serializer();
            ser.write_u8(4)?; //CreateInstance
            ser.write_string(&trait_name.0)?;
            ser.writer.flush()?;
            let mut deser = serder.get_deserializer();
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
            let mut ser = serder.get_serializer();
            ser.write_u8(5)?; //DropInstance
            let to = trait_object.as_usize_tuples();
            ser.write_usize(to.0)?;
            ser.write_usize(to.1)?;
            ser.writer.flush()?;
            let mut deser = serder.get_deserializer();
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

/// Represents a way to communicate across a TCP socket.
/// Possibilities include:
/// * Raw uncompressed communication
/// * TLS
/// * Zip-compression or similar
/// * Something else, or a combination of the above!
pub trait ConnectionStrategy {
    /// The type of serializer/deserializer used by the strategy.
    type TS : SerializerAndDeserializer + Send + 'static;
    /// Take ownership of, and wrap the given TcpStream in something
    /// that implements 'SerializerAndDeserializer'.
    fn create(&mut self, stream: TcpStream) -> Result<Self::TS, SavefileError>;
}

/// Represents a strategy of sending/receiving data without any encryption.
/// This type is only useful as a type argument to 'ClientConnection'.
/// It must always match the strategy used by the server.
pub struct UnencryptedStrategy;
impl ConnectionStrategy for UnencryptedStrategy {
    type TS = SeparateSerializerAndDeserializer<BufReader<TcpStream>, BufWriter<TcpStream>>;

    fn create(&mut self, stream: TcpStream) -> Result<Self::TS,SavefileError> {
        let ser = BufWriter::new(stream.try_clone().unwrap());
        let deser = BufReader::new(stream);

        let serder = SeparateSerializerAndDeserializer::new(
            deser, ser, 0, 0); //TODO: Support other versions!
        Ok(serder)
    }
}

/// Support for TLS (cryptography)
#[cfg(feature = "rustls")]
pub mod rustls;


impl<T:ConnectionStrategy> ClientConnection<T> {

    /// TODO: Optimize - add constructor that doesn't create new conn every time!
    pub fn new(addr: impl ToSocketAddrs, trait_name: &str, mut strategy: T) -> Result<ClientConnection<T>,SavefileError> {
        let mut stream = TcpStream::connect(addr)?;

        let serder = strategy.create(stream)?;

        Ok(ClientConnection::<T> {
            key: EntryKey {
                data1: 1<<63,
                data2: CONNECTION_ID.fetch_add(1, Ordering::Relaxed)
            },
            trait_name: TraitName(trait_name.into()),
            conn: Arc::new(Mutex::new(ClientConnectionState{
                serder,
            })),
        })
    }
}

unsafe impl<T:ConnectionStrategy> EntryPoint for ClientConnection<T> {
    unsafe fn call(&self, data: AbiProtocol) {
        let mut conn = self.conn.lock();
        let mut conn = &mut *conn;

        match process_client_command(&mut conn.serder, ClientCommand{
            input: data,
        }, &self.trait_name) {
            Ok(()) => {}
            Err(err) => {
                panic!("Failed to send command to remote: {:?}", err)
            }
        }
    }

    fn get_key(&self) -> EntryKey {
        self.key
    }
}

struct RemoteEntrypoint {

}

/// A handle to a running server.
/// When this is dropped, the server is stopped.
pub struct Server {
    addr: SocketAddr, //TODO: Replace this janky solution with a mechanism that implements select([accept_fd, local_pipe]) instead.
    jh: Option<JoinHandle<BackgroundListenerResult>>,
    has_quit: bool,
}

impl Server {
    /// Block forever. This will only return if the server fails.
    pub fn serve_forever(&mut self) -> BackgroundListenerResult {
        if let Some(x) = self.jh.take() {
            self.has_quit = true;
            let Ok(res) = x.join() else {
                return BackgroundListenerResult::UnexpectedError;
            };
            return res;
        } else {
            self.has_quit = true;
            return BackgroundListenerResult::AlreadyStopped
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        println!("TODO: Quit nicely");
        /*if !self.has_quit {
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
            //TODO: Quit!
        }
        _ = self.jh.take().map(|x|x.join());*/
    }
}
fn serve<S:ConnectionStrategy+Send+'static>(local: impl ToSocketAddrs + Send + 'static, supported_types: HashMap<TraitName, Arc<DynAbiExportableObjectType>>, mut strategy: S) -> Result<Server,SavefileError> {
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
                        println!("Failed to accept incoming connection: {:?}", err);
                        thread::sleep(Duration::from_millis(10));
                        continue 'rebind;
                    }
                };

                let types = supported_types.clone();
                let mut conn = match strategy.create(stream) {
                    Ok(conn) => {
                        conn
                    }
                    Err(err) => {
                        println!("Failed to accept incoming TLS-connection: {:?}", err);
                        continue;
                    }
                };
                thread::spawn(move|| {
                    match serve_connection(conn, types) {
                        Ok(_) => {},
                        Err(err) => {
                            println!("Worker error: {:?}", err);
                        }
                    }
                });
            }
        }
    });
    Ok(Server {
        addr,
        jh:Some(jh),
        has_quit: false
    })
}

/// Builder type for creating servers
pub struct ServerBuilder {
    traits: HashMap<TraitName, Arc<DynAbiExportableObjectType>>,
}
impl ServerBuilder {
    /// Create a new ServerBuilder.
    /// Use 'add_primary_trait' to add support for all constructable traits you wish to support.
    /// Use 'add_secondary_trait' to add support for trait objects returned by any trait function.
    pub fn new() -> ServerBuilder {
        ServerBuilder {
            traits: Default::default()
        }
    }
    /// Add support for a secondary trait type.
    /// This is needed if any of the primary traits contains a function that returns
    /// a boxed trait, or which takes a trait as an argument.
    pub fn add_secondary_trait<T:AbiExportable+?Sized>(mut self) -> Self {
        let latest_version = <T as AbiExportable>::get_latest_version();
        let name = <T as AbiExportable>::get_definition(latest_version).name;
        self.traits.insert(TraitName(name.clone()), Arc::new(DynAbiExportableObjectType{
            name: TraitName(name.into()),
            local_entry: <T as AbiExportable>::ABI_ENTRY,
            definitions: |ver|<T as AbiExportable>::get_definition(ver),
            latest_version: 0,
        }));
        self
    }
    /// Add support for the given concrete type 'T'.
    /// The type must be default-constructable (i.e, it must implement Default).
    /// The RPC-client will be able to create instances of this type.
    /// The savefile_abi_export!() -macro can be used to implement the required
    /// trait for a given implementation type.
    pub fn add_primary_trait<T:AbiExportableImplementation+?Sized>(mut self) -> Self {
        let latest_version = <T::AbiInterface as AbiExportable>::get_latest_version();
        let name = <T::AbiInterface as AbiExportable>::get_definition(latest_version).name;
        self.traits.insert(TraitName(name.clone()), Arc::new(DynAbiExportableObjectType{
            name: TraitName(name.into()),
            local_entry: <T as AbiExportableImplementation>::ABI_ENTRY,
            definitions: |ver|<T::AbiInterface as AbiExportable>::get_definition(ver),
            latest_version: 0,
        }));
        self
    }
    /// Finish constructing the server, start it, and return a handle to it.
    /// The server is stopped when the handle is dropped.
    pub fn finish<S:ConnectionStrategy+Send+'static>(self, local: impl ToSocketAddrs + Send + 'static, strategy: S) -> Result<Server, SavefileError> {
        serve::<S>(local, self.traits, strategy)
    }
}
