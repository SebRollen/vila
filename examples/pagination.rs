use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use stream_flatten_iters::TryStreamExt;
use vila::pagination::{PaginatedRequest, PaginationState, PaginationType, QueryPaginator};
use vila::{Client, Request, RequestData};

// Helpers
fn extract_page_number(q: &PaginationType) -> Option<usize> {
    if let PaginationType::Query(v) = q {
        v.first()
            .map(|(_, v)| str::parse::<usize>(v).ok())
            .flatten()
    } else {
        panic!("Unexpected paginator")
    }
}

fn get_next_url(
    prev: &PaginationState<PaginationType>,
    res: &PassengersWrapper,
) -> Option<Vec<(String, String)>> {
    let max_page = res.total_pages;
    let next_page = match prev {
        PaginationState::Start(None) => Some(1),
        PaginationState::Start(Some(x)) => extract_page_number(x).map(|x| x + 1),
        PaginationState::Next(x) => extract_page_number(x).map(|x| x + 1),
        PaginationState::End => None,
    };

    next_page
        .map(|page| if page > max_page { None } else { Some(page) })
        .flatten()
        .map(|page| vec![("page".into(), format!("{}", page))])
}

// Domain
#[derive(Serialize)]
struct GetPassengers {
    size: usize,
}

#[derive(Deserialize, Debug)]
struct Passenger {
    name: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct PassengersWrapper {
    total_passengers: usize,
    total_pages: usize,
    data: Vec<Passenger>,
}

impl Request for GetPassengers {
    type Data = Self;
    type Response = PassengersWrapper;

    fn endpoint(&self) -> Cow<str> {
        "/v1/passenger".into()
    }

    fn data(&self) -> RequestData<&Self> {
        RequestData::Query(self)
    }
}

impl PaginatedRequest for GetPassengers {
    type Paginator = QueryPaginator<Self::Response>;

    fn paginator(&self) -> Self::Paginator {
        QueryPaginator::new(get_next_url)
    }
}

#[tokio::main]
pub async fn main() {
    env_logger::init();
    let client = Client::new("https://api.instantwebtools.net");
    let req = GetPassengers { size: 1 };

    // Can send request individually
    println!("{:?}", client.send(&req).await);

    // Can send paginated request, returning stream of results
    client
        .send_paginated(&req)
        .map(|maybe_wrapper| maybe_wrapper.map(|wrapper| wrapper.data))
        .try_flatten_iters()
        .take(5)
        .for_each(|res| async move { println!("{:?}", res) })
        .await;
}
