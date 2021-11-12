use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use vila::pagination::path::*;
use vila::pagination::*;
use vila::{Client, Request};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request as MockRequest, ResponseTemplate};

#[derive(Clone)]
struct PathData {
    page: usize,
}

impl From<PathData> for PathUpdater {
    fn from(s: PathData) -> PathUpdater {
        let mut data = HashMap::new();
        // /nested/page/{number}
        //   ^      ^      ^
        //   0      1      2
        data.insert(2, s.page.to_string());
        PathUpdater { data }
    }
}

#[derive(Serialize)]
struct PaginationRequest {
    page: Option<usize>,
}

#[derive(Deserialize, Serialize, Debug)]
struct PaginationResponse {
    next_page: Option<usize>,
    data: String,
}

impl Request for PaginationRequest {
    type Data = ();
    type Response = PaginationResponse;

    fn endpoint(&self) -> Cow<str> {
        match self.page {
            Some(page) => format!("/nested/page/{}", page).into(),
            None => "/nested/page".into(),
        }
    }
}

impl PaginatedRequest for PaginationRequest {
    type Data = PathData;
    type Paginator = PathPaginator<PaginationResponse, PathData>;
    fn paginator(&self) -> Self::Paginator {
        PathPaginator::new(|_, r: &PaginationResponse| r.next_page.map(|page| PathData { page }))
    }
}

#[tokio::test]
async fn path_pagination() {
    let server = MockServer::start().await;
    let uri = server.uri();
    let client = Client::new(&uri);

    Mock::given(method("GET"))
        .and(path("/nested/page"))
        .respond_with(|_: &MockRequest| {
            let body = PaginationResponse {
                next_page: Some(1),
                data: "First!".into(),
            };
            ResponseTemplate::new(200).set_body_json(body)
        })
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/nested/page/1"))
        .respond_with(|_: &MockRequest| {
            let body = PaginationResponse {
                next_page: Some(2),
                data: "Second!".into(),
            };
            ResponseTemplate::new(200).set_body_json(body)
        })
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/nested/page/2"))
        .respond_with(|_: &MockRequest| {
            let body = PaginationResponse {
                next_page: None,
                data: "Last!".into(),
            };
            ResponseTemplate::new(200).set_body_json(body)
        })
        .mount(&server)
        .await;

    let mut response = client.send_paginated(&PaginationRequest { page: None });
    assert_eq!(
        response.next().await.unwrap().unwrap().data,
        "First!".to_string()
    );
    assert_eq!(
        response.next().await.unwrap().unwrap().data,
        "Second!".to_string()
    );
    assert_eq!(
        response.next().await.unwrap().unwrap().data,
        "Last!".to_string()
    );
    assert!(response.next().await.is_none());
}
