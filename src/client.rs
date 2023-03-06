use crate::error::{Error, Result};
use crate::pagination::{PaginatedRequest, Paginator, RequestModifier, State};
use crate::request::{Request, RequestBuilderExt};
use futures::prelude::*;
#[cfg(feature = "progress")]
use indicatif::{MultiProgress, ProgressBar};
use log::debug;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Client as ReqwestClient;
use std::convert::TryFrom;
use std::sync::Arc;

#[derive(Clone)]
enum Authorization {
    Bearer(String),
    Basic(String, Option<String>),
    Query(Vec<(String, String)>),
    Header(HeaderMap<HeaderValue>),
}

/// The main client used for making requests.
///
/// `Client` stores an async Reqwest client as well as the associated
/// base url and possible authorization details for the REST server.
#[derive(Clone)]
pub struct Client {
    inner: Arc<ReqwestClient>,
    base_url: String,
    auth: Option<Authorization>,
    #[cfg(feature = "progress")]
    progress: Option<Arc<MultiProgress>>,
}

impl Client {
    /// Create a new `Client`.
    pub fn new<S: ToString>(base_url: S) -> Self {
        let client = ReqwestClient::new();

        Self::from_reqwest(client, base_url)
    }

    /// Create a new `Client` from an existing Reqwest Client.
    pub fn from_reqwest<S: ToString>(client: ReqwestClient, base_url: S) -> Self {
        let inner = Arc::new(client);

        Self {
            inner,
            base_url: base_url.to_string(),
            auth: None,
            #[cfg(feature = "progress")]
            progress: None,
        }
    }

    #[cfg(feature = "progress")]
    /// Display a progress bar for paginated requests.
    /// If progress is shown, the URL for each request will be printed to the command line to
    /// indicate the current request(s), so care must be taken in case the URL includes sensitive
    /// details such as API keys.
    pub fn show_progress(mut self) -> Self {
        self.progress = Some(Arc::new(MultiProgress::new()));
        self
    }

    /// Enable bearer authentication for the client
    pub fn bearer_auth<S: ToString>(mut self, token: S) -> Self {
        self.auth = Some(Authorization::Bearer(token.to_string()));
        self
    }

    /// Enable basic authentication for the client
    pub fn basic_auth<T: Into<Option<S>>, S: ToString>(mut self, user: S, pass: T) -> Self {
        self.auth = Some(Authorization::Basic(
            user.to_string(),
            pass.into().map(|x| x.to_string()),
        ));
        self
    }

    /// Enable query authentication for the client
    pub fn query_auth<S: ToString>(mut self, pairs: Vec<(S, S)>) -> Self {
        let pairs = pairs
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        self.auth = Some(Authorization::Query(pairs));
        self
    }

    /// Enable custom header authentication for the client
    pub fn header_auth<S: ToString>(mut self, pairs: Vec<(S, S)>) -> Self {
        let mut map = HeaderMap::new();
        for (k, v) in pairs {
            let k = k.to_string();
            let v = v.to_string();
            let mut header_value = HeaderValue::from_str(&v).expect("Failed to create HeaderValue");
            header_value.set_sensitive(true);
            map.insert(
                HeaderName::try_from(&k).expect("Failed to create HeaderName"),
                header_value,
            );
        }
        self.auth = Some(Authorization::Header(map));
        self
    }

    fn format_request<R: Request>(&self, request: &R) -> Result<reqwest::Request> {
        let endpoint = request.endpoint();
        let endpoint = endpoint.trim_matches('/');
        let url = format!("{}/{}", self.base_url, endpoint);

        let req = self
            .inner
            .request(R::METHOD, url)
            .headers(request.headers())
            .request_data(request.data());

        let req = match &self.auth {
            None => req,
            Some(Authorization::Bearer(token)) => req.bearer_auth(token),
            Some(Authorization::Basic(user, pass)) => req.basic_auth(user, pass.as_ref()),
            Some(Authorization::Query(pairs)) => req.query(&pairs),
            Some(Authorization::Header(pairs)) => req.headers(pairs.clone()),
        };
        req.build().map_err(From::from)
    }

    fn send_raw<R>(&self, req: reqwest::Request) -> impl Future<Output = Result<R>>
    where
        R: for<'de> serde::Deserialize<'de>,
    {
        debug!("Sending request: {:?}", req);
        self.inner
            .execute(req)
            .map_err(From::from)
            .and_then(|res| async {
                let status = res.status();
                if status.is_success() {
                    res.json().await.map_err(From::from)
                } else if status.is_client_error() {
                    Err(Error::ClientError(status, res.text().await.unwrap()))
                } else {
                    Err(Error::ServerError(status, res.text().await.unwrap()))
                }
            })
    }

    /// Send a single `Request`
    pub async fn send<R: Request>(&self, request: &R) -> Result<R::Response> {
        let req = self.format_request(request)?;
        self.send_raw(req).await
    }

    /// Send a paginated request, returning a stream of results
    pub fn send_paginated<'a, R: PaginatedRequest>(
        &'a self,
        request: &'a R,
    ) -> impl Stream<Item = Result<R::Response>> + Unpin + 'a {
        #[cfg(feature = "progress")]
        let progress = self
            .progress
            .as_ref()
            .map(|m| m.add(ProgressBar::new_spinner()));
        Box::pin(stream::try_unfold(
            (
                request.paginator(),
                State::Start(request.initial_page()),
                #[cfg(feature = "progress")]
                progress,
            ),
            move |x| async move {
                #[cfg(feature = "progress")]
                let (paginator, state, progress) = x;

                #[cfg(not(feature = "progress"))]
                let (paginator, state) = x;

                let mut base_request = self.format_request(request)?;
                let page = match state {
                    State::Start(None) => None,
                    State::Start(Some(ref page)) | State::Next(ref page) => Some(page),
                    State::End => {
                        #[cfg(feature = "progress")]
                        if let Some((p, m)) = progress.zip(self.progress.as_ref()) {
                            p.finish_and_clear();
                            m.remove(&p);
                        }
                        return Ok(None);
                    }
                };
                if let Some(page) = page {
                    let modifier = paginator.modifier(page.clone());
                    modifier.modify_request(&mut base_request)?;
                }
                #[cfg(feature = "progress")]
                if let Some(p) = progress.as_ref() {
                    p.set_message(base_request.url().to_string())
                }
                let response = self.send_raw(base_request).await?;
                let state = paginator.next(page, &response);
                #[cfg(feature = "progress")]
                if let Some(ref p) = progress {
                    p.tick();
                }
                Ok(Some((
                    response,
                    (
                        paginator,
                        state,
                        #[cfg(feature = "progress")]
                        progress,
                    ),
                )))
            },
        ))
    }
}
