//! Constructs for wrapping a paginated API.
use crate::error::{Error, Result};
use crate::Request;
use reqwest::Request as RawRequest;
use std::collections::HashMap;

/// Trait for updating an HTTP request with pagination data.
pub trait RequestModifier {
    /// Modify the request with updated pagination data.
    fn modify_request(&self, request: &mut RawRequest) -> Result<()>;
}

/// Base trait for paginators. Paginators can use the previous pagination state
/// and the response from the previous request to create a new pagination state.
pub trait Paginator<T, U> {
    /// The associated modifier that modifies the request with new pagination data.
    type Modifier: RequestModifier;

    /// Constructs an associated modifier using pagination data.
    fn modifier(&self, data: U) -> Self::Modifier;
    /// Method for returning the next pagination state given the previous pagination data and the results from the previous request.
    fn next(&self, prev: Option<&U>, res: &T) -> State<U>;
}

/// Trait for any request that requires pagination.
pub trait PaginatedRequest: Request {
    /// Associated data that can be used for pagination.
    type Data: Clone;

    /// The paginator used for the request.
    type Paginator: Paginator<Self::Response, <Self as PaginatedRequest>::Data>;

    /// Return the associated paginator.
    fn paginator(&self) -> Self::Paginator;

    /// Specify the initial page to start pagination from. Defaults to `None`, which means
    /// pagination will begin from whatever page the API defines as the initial page.
    fn initial_page(&self) -> Option<<Self as PaginatedRequest>::Data> {
        None
    }
}

#[derive(Clone, Debug)]
/// The current pagination state.
pub enum State<T> {
    /// State associated with the initial request.
    Start(Option<T>),
    /// State associated with continuing pagination.
    Next(T),
    /// State denoting that the last page has been reached.
    End,
}

impl<T> Default for State<T> {
    fn default() -> State<T> {
        State::Start(None)
    }
}

pub mod query {
    //! Constructs for working with APIs that implement paging through one or more query parameters.
    use super::*;
    #[derive(Debug, Clone)]
    /// A modifier that updates the query portion of a request's URL. This modifier updates the
    /// query keys using the values inside the data HashMap, overwriting any existing fields and
    /// appending any non-existing fields.
    pub struct QueryModifier {
        pub data: HashMap<String, String>,
    }

    impl RequestModifier for QueryModifier {
        fn modify_request(&self, request: &mut RawRequest) -> Result<()> {
            let url = request.url_mut();
            let unchanged_queries: Vec<(_, _)> = url
                .query_pairs()
                .filter(|(k, _)| !self.data.contains_key(k.as_ref()))
                .collect();
            let mut temp_url = url.clone();
            temp_url.set_query(None);
            for (key, val) in unchanged_queries {
                temp_url.query_pairs_mut().append_pair(&key, &val);
            }
            for (key, val) in self.data.iter() {
                temp_url.query_pairs_mut().append_pair(key, val);
            }
            url.set_query(temp_url.query());
            Ok(())
        }
    }

    /// A paginator that implements pagination through one or more query parameters.
    pub struct QueryPaginator<T, U> {
        f: Box<dyn 'static + Send + Sync + Fn(Option<&U>, &T) -> Option<U>>,
    }

    impl<T, U> QueryPaginator<T, U> {
        pub fn new<F: 'static + Send + Sync + Fn(Option<&U>, &T) -> Option<U>>(f: F) -> Self {
            Self { f: Box::new(f) }
        }
    }

    impl<T, U> Paginator<T, U> for QueryPaginator<T, U>
    where
        U: Into<QueryModifier>,
    {
        type Modifier = QueryModifier;

        fn modifier(&self, data: U) -> QueryModifier {
            data.into()
        }

        fn next(&self, prev: Option<&U>, res: &T) -> State<U> {
            let queries = (self.f)(prev, res);
            match queries {
                Some(queries) => State::Next(queries),
                None => State::End,
            }
        }
    }
}

pub mod path {
    //! Constructs for working with APIs that implement paging through one or more path parameters.
    use super::*;
    #[derive(Debug, Clone)]

    /// A modifier that updates the path portion of a request's URL. This modifier holds a HashMap
    /// that maps the position of a path parameter to its updated value.
    pub struct PathModifier {
        pub data: HashMap<usize, String>,
    }

    impl RequestModifier for PathModifier {
        fn modify_request(&self, request: &mut RawRequest) -> Result<()> {
            let url = request.url_mut();
            let temp_url = url.clone();
            let mut new_segments: Vec<&str> = temp_url
                .path_segments()
                .ok_or_else(|| Error::Pagination {
                    msg: "URL cannot be a base".to_string(),
                })?
                .enumerate()
                .map(|(i, x)| self.data.get(&i).map(|val| val.as_str()).unwrap_or(x))
                .collect();
            let len = new_segments.len();
            // Append any additional path segments not present in original path
            new_segments.extend(self.data.iter().filter_map(|(i, x)| {
                if *i >= len {
                    Some(x.as_str())
                } else {
                    None
                }
            }));
            let mut path_segments = url.path_segments_mut().map_err(|_| Error::Pagination {
                msg: "URL cannot be a base".to_string(),
            })?;
            path_segments.clear();
            path_segments.extend(new_segments.iter());
            Ok(())
        }
    }

    /// A paginator that implements pagination through one or more path parameters. The closure inside
    /// the paginator should return the path segment number and the new path segment, e.g. (2, "foo")
    /// represents changing the third path segment to "foo"
    pub struct PathPaginator<T, U> {
        f: Box<dyn 'static + Send + Sync + Fn(Option<&U>, &T) -> Option<U>>,
    }

    impl<T, U> PathPaginator<T, U> {
        pub fn new<F: 'static + Send + Sync + Fn(Option<&U>, &T) -> Option<U>>(f: F) -> Self {
            Self { f: Box::new(f) }
        }
    }

    impl<T, U> Paginator<T, U> for PathPaginator<T, U>
    where
        U: Into<PathModifier>,
    {
        type Modifier = PathModifier;
        fn modifier(&self, data: U) -> Self::Modifier {
            data.into()
        }
        fn next(&self, prev: Option<&U>, res: &T) -> State<U> {
            let path = (self.f)(prev, res);
            match path {
                Some(path) => State::Next(path),
                None => State::End,
            }
        }
    }
}
