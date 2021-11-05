use crate::utils::EmptyHello;
use vila::Client;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn bearer_auth() {
    let server = MockServer::start().await;
    let uri = server.uri();
    let client = Client::new(&uri).bearer_auth("PASSWORD");

    Mock::given(method("GET"))
        .and(path("/hello"))
        .and(header("Authorization", "Bearer PASSWORD"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    client.send(&EmptyHello).await.unwrap();
}
