//! External-style compile qualification for the native Tower service adapter.

use bytes::Bytes;
use eggfetch_core::{Client, NativeHttpService};
use http_body::Body;
use tonic::body::BoxBody;
use tonic::client::{Grpc, GrpcService};

fn assert_tonic_transport<T>(service: T)
where
    T: GrpcService<BoxBody>,
    T::ResponseBody: Body<Data = Bytes> + Send + 'static,
    T::Error: Into<tonic::codegen::StdError>,
{
    let _client = Grpc::with_origin(service, http::Uri::from_static("http://127.0.0.1:50051"));
}

fn main() {
    let service: NativeHttpService = Client::new().native_service();
    assert_tonic_transport(service);
    println!("native tower service satisfies tonic transport bounds");
}
