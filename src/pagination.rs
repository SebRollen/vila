use crate::Request;

#[derive(Debug, Clone)]
/// The type of pagination used for the resource.
pub enum PaginationType {
    /// Pagination by one or multiple query parameters.
    Query(Vec<(String, String)>),
    Path(Vec<(usize, String)>),
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
#[allow(clippy::type_complexity)]
pub struct QueryPaginator<T> {
    f: Box<dyn Fn(&PaginationState<PaginationType>, &T) -> Option<Vec<(String, String)>>>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> QueryPaginator<T> {
    pub fn new<
        F: 'static + Fn(&PaginationState<PaginationType>, &T) -> Option<Vec<(String, String)>>,
    >(
        f: F,
    ) -> Self {
        Self {
            f: Box::new(f),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T> Paginator<T> for QueryPaginator<T> {
    fn next(
        &self,
        prev: &PaginationState<PaginationType>,
        res: &T,
    ) -> PaginationState<PaginationType> {
        let queries = (self.f)(prev, res);
        match queries {
            Some(queries) => PaginationState::Next(PaginationType::Query(queries)),
            None => PaginationState::End,
        }
    }
}

/// A paginator that implements pagination through one or more path parameters. The closure inside
/// the paginator should return the path segment number and the new path segment, e.g. (2, "foo")
/// represents changing the third path segment to "foo"
#[allow(clippy::type_complexity)]
pub struct PathPaginator<T> {
    f: Box<dyn Fn(&PaginationState<PaginationType>, &T) -> Option<Vec<(usize, String)>>>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> PathPaginator<T> {
    pub fn new<
        F: 'static + Fn(&PaginationState<PaginationType>, &T) -> Option<Vec<(usize, String)>>,
    >(
        f: F,
    ) -> Self {
        Self {
            f: Box::new(f),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T> Paginator<T> for PathPaginator<T> {
    fn next(
        &self,
        prev: &PaginationState<PaginationType>,
        res: &T,
    ) -> PaginationState<PaginationType> {
        let path = (self.f)(prev, res);
        match path {
            Some(path) => PaginationState::Next(PaginationType::Path(path)),
            None => PaginationState::End,
        }
    }
}
