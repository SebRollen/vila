use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use stream_flatten_iters::TryStreamExt;
use vila::pagination::{PaginatedRequest, PaginationState, QueryPaginator, QueryUpdater};
use vila::{Client, Request, RequestData};

// Domain
#[derive(Clone)]
struct PaginationData {
    page: usize,
}

impl From<PaginationData> for QueryUpdater {
    fn from(s: PaginationData) -> QueryUpdater {
        let mut data = HashMap::new();
        data.insert("page".into(), s.page.to_string());
        QueryUpdater { data }
    }
}

#[derive(Serialize)]
struct GetPassengers {
    size: usize,
    page: Option<usize>,
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
    type PaginationData = PaginationData;
    type Paginator = QueryPaginator<Self::Response, PaginationData>;

    fn initial_page(&self) -> Option<Self::PaginationData> {
        self.page.map(|page| PaginationData { page })
    }

    fn paginator(&self) -> Self::Paginator {
        QueryPaginator::new(
            |prev: &PaginationState<PaginationData>, res: &PassengersWrapper| {
                let max_page = res.total_pages;
                match prev {
                    PaginationState::Start(None) => Some(PaginationData { page: 1 }),
                    PaginationState::Start(Some(x)) | PaginationState::Next(x) => {
                        if x.page == max_page {
                            None
                        } else {
                            Some(PaginationData { page: x.page + 1 })
                        }
                    }
                    PaginationState::End => None,
                }
            },
        )
    }
}

#[tokio::main]
pub async fn main() {
    env_logger::init();
    let client = Client::new("https://api.instantwebtools.net");
    let req = GetPassengers {
        page: None,
        size: 1,
    };

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
