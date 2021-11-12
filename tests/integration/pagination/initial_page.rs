use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use vila::pagination::*;
use vila::{Client, Request, RequestData};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, Request as MockRequest, ResponseTemplate};

struct QueryData {
    page: usize,
}

impl ToQueryPagination for QueryData {
    fn to_query_pagination(&self) -> HashMap<String, String> {
        let mut h = HashMap::new();
        h.insert("page".into(), self.page.to_string());
        h
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
    type Data = Self;
    type Response = PaginationResponse;

    fn endpoint(&self) -> Cow<str> {
        "/page".into()
    }

    fn data(&self) -> RequestData<&Self> {
        RequestData::Query(self)
    }
}

impl PaginatedRequest for PaginationRequest {
    type Paginator = QueryPaginator<PaginationResponse, QueryData>;
    fn paginator(&self) -> Self::Paginator {
        QueryPaginator::new(|_, r: &PaginationResponse| r.next_page.map(|page| QueryData { page }))
    }
    fn initial_page(&self) -> Option<PaginationType> {
        self.page.map(|page| {
            let mut h = HashMap::new();
            h.insert("page".into(), page.to_string());
            PaginationType::Query(h)
        })
    }
}

#[tokio::test]
async fn query_pagination() {
    let server = MockServer::start().await;
    let uri = server.uri();
    let client = Client::new(&uri);

    Mock::given(method("GET"))
        .and(path("/page"))
        .and(query_param("page", "1"))
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
        .and(path("/page"))
        .and(query_param("page", "2"))
        .respond_with(|_: &MockRequest| {
            let body = PaginationResponse {
                next_page: None,
                data: "Last!".into(),
            };
            ResponseTemplate::new(200).set_body_json(body)
        })
        .mount(&server)
        .await;

    let mut response = client.send_paginated(&PaginationRequest { page: Some(1) });
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
