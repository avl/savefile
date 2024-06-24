use savefile_abi::abi_remote::{ClientConnection, ServerBuilder, UnencryptedStrategy};
use savefile_abi::{AbiConnection, Owning};
use savefile_abi::abi_remote::rustls::{TlsClientStrategy, TlsServerStrategy};

#[savefile_abi_exportable(version=0)]
pub trait TestTrait {
    fn call_trait(&self, x: u32, y: u32) -> u32;
}
#[derive(Default)]
struct TestTraitImpl;
impl TestTrait for TestTraitImpl {
    fn call_trait(&self, x:u32, y: u32) -> u32 {
        x+y
    }
}

savefile_abi_export!(TestTraitImpl, TestTrait);



#[test]
fn test_server() {
    let _jh = ServerBuilder::new()
        .add_primary_trait::<TestTraitImpl>()
        .finish("127.0.0.1:1234", UnencryptedStrategy);

    let conn = ClientConnection::<UnencryptedStrategy>::new("127.0.0.1:1234", "TestTrait", UnencryptedStrategy).unwrap();
    let abi_conn = AbiConnection::<dyn TestTrait, _>::from_entrypoint(conn, None, Owning::Owned).unwrap();
    let x = abi_conn.call_trait(40, 2);
    assert_eq!(x, 42);
}
#[test]
fn test_tls_server() {
    let tls_server = TlsServerStrategy::new(
        "testkeys/client.crt",
        "testkeys/client.key").unwrap();
    let tls_client = TlsClientStrategy::new("testkeys/root.crt", "localhost").unwrap();
    let _jh = ServerBuilder::new()
        .add_primary_trait::<TestTraitImpl>()
        .finish("localhost:1234", tls_server);

    let conn = ClientConnection::new("127.0.0.1:1234", "TestTrait", tls_client).unwrap();
    let abi_conn = AbiConnection::<dyn TestTrait, _>::from_entrypoint(conn, None, Owning::Owned).unwrap();
    abi_conn.call_trait(40,2);
}