//! External-style compile qualification for the native Tower service adapter.

use bytes::Bytes;
use eggfetch_core::{Client, NativeHttpService};
use http_body::Body as HttpBody;
use tonic::body::Body as TonicBody;
use tonic::client::{Grpc, GrpcService};
use tonic::codegen::StdError;

fn assert_tonic_014_transport<T>(service: T)
where
    T: GrpcService<TonicBody>,
    T::Error: Into<StdError>,
    T::ResponseBody: HttpBody<Data = Bytes> + Send + 'static,
    <T::ResponseBody as HttpBody>::Error: Into<StdError> + Send,
{
    let _client = Grpc::with_origin(service, http::Uri::from_static("http://127.0.0.1:50051"));
}

fn main() {
    let service: NativeHttpService = Client::new().native_service();
    assert_tonic_014_transport(service);
    println!("native tower service satisfies tonic 0.14 transport bounds");
}
