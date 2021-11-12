use crate::Request;
use reqwest::Url;
use std::collections::HashMap;

pub trait UrlUpdater {
    fn update_url(&self, url: &mut Url);
}

#[derive(Debug, Clone)]
pub struct QueryUpdater {
    pub data: HashMap<String, String>,
}

impl UrlUpdater for QueryUpdater {
    fn update_url(&self, url: &mut Url) {
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
    }
}

#[derive(Debug, Clone)]
pub struct PathUpdater {
    pub data: HashMap<usize, String>,
}

impl UrlUpdater for PathUpdater {
    fn update_url(&self, url: &mut Url) {
        let temp_url = url.clone();
        let mut new_segments: Vec<&str> = temp_url
            .path_segments()
            .expect("URL cannot be a base")
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
        url.path_segments_mut().unwrap().clear();
        url.path_segments_mut().unwrap().extend(new_segments.iter());
    }
}

/// Base trait for paginators. Paginators can use the previous pagination state
/// and the response from the previous request to create a new pagination state.
pub trait Paginator<T, U> {
    type Updater: UrlUpdater;

    fn updater(&self, data: U) -> Self::Updater;
    fn next(&self, prev: &PaginationState<U>, res: &T) -> PaginationState<U>;
}

/// Trait for any request that requires pagination.
pub trait PaginatedRequest: Request {
    type PaginationData: Clone;
    /// The paginator used for the request.
    type Paginator: Paginator<Self::Response, Self::PaginationData>;

    /// Return the associated paginator.
    fn paginator(&self) -> Self::Paginator;
    /// Specify the initial page to start pagination from. Defaults to `None`, which means
    /// pagination will begin from whatever page the API defines as the initial page.
    fn initial_page(&self) -> Option<Self::PaginationData> {
        None
    }
}

#[derive(Clone, Debug)]
/// The current pagination state.
pub enum PaginationState<T> {
    /// State associated with the initial request.
    Start(Option<T>),
    /// State associated with continuing pagination.
    Next(T),
    /// State denoting that the last page has been reached.
    End,
}

impl<T> Default for PaginationState<T> {
    fn default() -> PaginationState<T> {
        PaginationState::Start(None)
    }
}

/// A paginator that implements pagination through one or more query parameters.
pub struct QueryPaginator<T, U> {
    f: Box<dyn Fn(&PaginationState<U>, &T) -> Option<U>>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, U> QueryPaginator<T, U> {
    pub fn new<F: 'static + Fn(&PaginationState<U>, &T) -> Option<U>>(f: F) -> Self {
        Self {
            f: Box::new(f),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, U> Paginator<T, U> for QueryPaginator<T, U>
where
    U: Into<QueryUpdater>,
{
    type Updater = QueryUpdater;

    fn updater(&self, data: U) -> QueryUpdater {
        data.into()
    }

    fn next(&self, prev: &PaginationState<U>, res: &T) -> PaginationState<U> {
        let queries = (self.f)(prev, res);
        match queries {
            Some(queries) => PaginationState::Next(queries),
            None => PaginationState::End,
        }
    }
}

/// A paginator that implements pagination through one or more path parameters. The closure inside
/// the paginator should return the path segment number and the new path segment, e.g. (2, "foo")
/// represents changing the third path segment to "foo"
pub struct PathPaginator<T, U> {
    f: Box<dyn Fn(&PaginationState<U>, &T) -> Option<U>>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, U> PathPaginator<T, U> {
    pub fn new<F: 'static + Fn(&PaginationState<U>, &T) -> Option<U>>(f: F) -> Self {
        Self {
            f: Box::new(f),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, U> Paginator<T, U> for PathPaginator<T, U>
where
    U: Into<PathUpdater>,
{
    type Updater = PathUpdater;
    fn updater(&self, data: U) -> Self::Updater {
        data.into()
    }
    fn next(&self, prev: &PaginationState<U>, res: &T) -> PaginationState<U> {
        let path = (self.f)(prev, res);
        match path {
            Some(path) => PaginationState::Next(path),
            None => PaginationState::End,
        }
    }
}
