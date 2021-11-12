use crate::Request;
use std::collections::HashMap;

#[derive(Debug, Clone)]
/// The type of pagination used for the resource.
pub enum PaginationType {
    /// Pagination by one or multiple query parameters.
    Query(HashMap<String, String>),
    /// Pagination by one or multiple path parameters.
    Path(HashMap<usize, String>),
}

/// Base trait for paginators. Paginators can use the previous pagination state
/// and the response from the previous request to create a new pagination state.
pub trait Paginator<T> {
    fn next(
        &self,
        prev: &PaginationState<PaginationType>,
        res: &T,
    ) -> PaginationState<PaginationType>;
}

/// Trait for any request that requires pagination.
pub trait PaginatedRequest: Request {
    /// The paginator used for the request.
    type Paginator: Paginator<Self::Response>;

    /// Return the associated paginator.
    fn paginator(&self) -> Self::Paginator;
    /// Specify the initial page to start pagination from. Defaults to `None`, which means
    /// pagination will begin from whatever page the API defines as the initial page.
    fn initial_page(&self) -> Option<PaginationType> {
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

pub trait ToQueryPagination {
    fn to_query_pagination(&self) -> HashMap<String, String>;
}

impl ToQueryPagination for HashMap<String, String> {
    fn to_query_pagination(&self) -> HashMap<String, String> {
        self.clone()
    }
}

/// A paginator that implements pagination through one or more query parameters.
#[allow(clippy::type_complexity)]
pub struct QueryPaginator<T, U> {
    f: Box<dyn Fn(&PaginationState<PaginationType>, &T) -> Option<U>>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, U> QueryPaginator<T, U> {
    pub fn new<F: 'static + Fn(&PaginationState<PaginationType>, &T) -> Option<U>>(f: F) -> Self {
        Self {
            f: Box::new(f),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, U> Paginator<T> for QueryPaginator<T, U>
where
    U: ToQueryPagination,
{
    fn next(
        &self,
        prev: &PaginationState<PaginationType>,
        res: &T,
    ) -> PaginationState<PaginationType> {
        let queries = (self.f)(prev, res).map(|res| res.to_query_pagination());
        match queries {
            Some(queries) => PaginationState::Next(PaginationType::Query(queries)),
            None => PaginationState::End,
        }
    }
}

pub trait ToPathPagination {
    fn to_path_pagination(&self) -> HashMap<usize, String>;
}

impl ToPathPagination for HashMap<usize, String> {
    fn to_path_pagination(&self) -> HashMap<usize, String> {
        self.clone()
    }
}

/// A paginator that implements pagination through one or more path parameters. The closure inside
/// the paginator should return the path segment number and the new path segment, e.g. (2, "foo")
/// represents changing the third path segment to "foo"
#[allow(clippy::type_complexity)]
pub struct PathPaginator<T, U> {
    f: Box<dyn Fn(&PaginationState<PaginationType>, &T) -> Option<U>>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, U> PathPaginator<T, U> {
    pub fn new<F: 'static + Fn(&PaginationState<PaginationType>, &T) -> Option<U>>(f: F) -> Self {
        Self {
            f: Box::new(f),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, U> Paginator<T> for PathPaginator<T, U>
where
    U: ToPathPagination,
{
    fn next(
        &self,
        prev: &PaginationState<PaginationType>,
        res: &T,
    ) -> PaginationState<PaginationType> {
        let path = (self.f)(prev, res).map(|res| res.to_path_pagination());
        match path {
            Some(path) => PaginationState::Next(PaginationType::Path(path)),
            None => PaginationState::End,
        }
    }
}
