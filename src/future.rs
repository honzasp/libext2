use std::boxed::{FnBox};
use std::cell::{RefCell, RefMut};
use std::rc::{Rc};

#[must_use]
/// Promise is a handle to resolve a future.
///
/// When a `Promise` is resolved, the value will be available in the matching
/// `Future`. The `Promise` can be thought of as the "write-end" of the pipe.
pub struct Promise<A, X> {
  state: Rc<RefCell<State<A, X>>>,
}

#[must_use]
/// Future represents a value that could be available in the future.
pub struct Future<A, X> {
  state: Rc<RefCell<State<A, X>>>,
}

/// Shared state of a `Promise` and a `Future`.
///
/// The state is needed to connect the result and the binding, because one of
/// them comes earlier. If the promise is resolved before the binding of the
/// future is determined, the value is stored in `result`. If, on the other
/// hand, the binding is defined before the result is computed, we need to store
/// the binding in order to invoke it later, when the result becomes available.
///
/// `State` is stored in an `Rc<RefCell<...>>` and there are at most two owners,
/// the `Promise` and the `Future`. Thus, when the `Future` finds out that it is
/// the only owner (`Rc::try_unwrap` succeeds), the `Promise` at the other end
/// must have already written its value into the state. Correspondingly, when a
/// `Promise` is left as a single owner of the state, the `Future` must have
/// already bound a function as the listener to the new value. This invariant
/// can be broken when a `Future` or `Promise` is disposed of without consuming
/// or resolving, which results in a thread panic.
struct State<A, X> {
  binding: Option<Box<FnBox(Result<A, X>)>>,
  result: Option<Result<A, X>>,
}

/// Ensures that the `binding` is called when or if the `state` is resolved.
///
/// If the `state` already contains a result, we invoke the binding directly
/// (in this case, no heap allocation needs to be performed). Otherwise, the
/// binding is stored in the `state`, waiting for the result to come.
fn bind<A, X, F: 'static>(state: Rc<RefCell<State<A, X>>>, binding: F)
  where F: FnOnce(Result<A, X>) 
{
  match Rc::try_unwrap(state) {
    Ok(ref_cell) => {
      let state = ref_cell.into_inner();
      let result = state.result.unwrap();
      assert!(state.binding.is_none());
      binding(result);
    },
    Err(rc) => {
      let mut state = (*rc).borrow_mut();
      assert!(state.binding.is_none());
      assert!(state.result.is_none());
      (*state).binding = Some(Box::new(binding));
    },
  }
}

/// Ensures that the `result` is passed to the function that will be or already
/// is bound to the `state`.
///
/// If there is already a binding waiting for a result, we call it directly.
/// Otherwise, the `result` is stored inside the `state`, where it will wait
/// until the state is bound.
fn resolve<A, X>(state: Rc<RefCell<State<A, X>>>,
  result: Result<A, X>)
{
  match Rc::try_unwrap(state) {
    Ok(ref_cell) => {
      let state = ref_cell.into_inner();
      let binding = state.binding.unwrap();
      assert!(state.result.is_none());
      binding(result);
    },
    Err(rc) => {
      let mut state = (*rc).borrow_mut();
      assert!(state.binding.is_none());
      assert!(state.result.is_none());
      (*state).result = Some(result);
    },
  }
}

/// Arranges for the `write_promise` to resolve to the value of `read_future`
/// when it becomes available.
///
/// This operation requires additional heap allocation only if the state of
/// `write_promise` is unbound and the state of `read_future` is unresolved at
/// the same time. Otherwise, we can simply invoke the known binding with the
/// known result, move the known result to the state of `write_promise`, where
/// the unknown binding will read it, or move the known binding to the state of
/// `read_future`, where it will be invoked when this state gets resolved.
fn attach<A: 'static, X: 'static>(
  write_promise: Promise<A, X>,
  read_future: Future<A, X>) 
{
  assert!(write_promise.state.borrow().result.is_none());
  assert!(read_future.state.borrow().binding.is_none());

  match (Rc::try_unwrap(write_promise.state), Rc::try_unwrap(read_future.state)) {
    (Ok(write_cell), Ok(read_cell)) => {
      let write_state: State<A, X> = write_cell.into_inner();
      let read_state: State<A, X> = read_cell.into_inner();
      (write_state.binding.unwrap())(read_state.result.unwrap());
    },
    (Ok(write_cell), Err(read_rc)) => {
      let write_state: State<A, X> = write_cell.into_inner();
      let mut read_state: RefMut<State<A, X>> = (*read_rc).borrow_mut();
      assert!(write_state.binding.is_some());
      assert!(read_state.result.is_none());
      read_state.binding = write_state.binding;
    },
    (Err(write_rc), Ok(read_cell)) => {
      let mut write_state: RefMut<State<A, X>> = (*write_rc).borrow_mut();
      let read_state: State<A, X> = read_cell.into_inner();
      assert!(write_state.binding.is_none());
      assert!(read_state.result.is_some());
      write_state.result = read_state.result;
    },
    (Err(write_rc), Err(read_rc)) => {
      let mut read_state: RefMut<State<A, X>> = (*read_rc).borrow_mut();
      assert!(read_state.result.is_none());
      read_state.binding = Some(Box::new(move |result| resolve(write_rc, result)));
    },
  }
}

impl<A, X> Promise<A, X> {
  /// Creates a bound pair of a Promise and a Future.
  /// The result that resolves the promise will become the value of the future.
  pub fn new() -> (Promise<A, X>, Future<A, X>) {
    let state = Rc::new(RefCell::new(State {
      binding: None,
      result: None,
    }));

    let promise = Promise { state: state.clone() };
    let future = Future { state: state };
    (promise, future)
  }

  pub fn resolve(self, result: Result<A, X>) {
    resolve(self.state, result)
  }
  pub fn resolve_ok(self, value: A) {
    resolve(self.state, Ok(value))
  }
  pub fn resolve_err(self, error: X) {
    resolve(self.state, Err(error))
  }
}

impl<A: 'static, X: 'static> Future<A, X> {
  /// Creates a Future that is resolved to `result`.
  pub fn new(result: Result<A, X>) -> Future<A, X> {
    let state = Rc::new(RefCell::new(State {
      binding: None,
      result: Some(result),
    }));
    Future { state: state }
  }

  pub fn new_ok(value: A) -> Future<A, X> {
    Self::new(Ok(value))
  }
  pub fn new_err(error: X) -> Future<A, X> {
    Self::new(Err(error))
  }

  /// Calls the `callback` when the value of `self` becomes available.
  pub fn consume<F: 'static>(self, callback: F) 
    where F: FnOnce(Result<A, X>) 
  {
    bind(self.state, callback)
  }
  
  /// Calls the `callback` when the value of `self` becomes avaiable, but only
  /// if it is an error.
  pub fn consume_error<F: 'static>(self, callback: F) 
    where F: FnOnce(X) 
  {
    bind(self.state, |result| match result {
      Ok(_) => (),
      Err(error) => callback(error),
    })
  }

  /// When the value of `self` becomes available and it is `Ok`, calls the
  /// `callback`. The future returned from this method is then resolved to the
  /// value of the future returned from the callback.
  pub fn then<F: 'static, B: 'static>(self, callback: F) -> Future<B, X>
    where F: FnOnce(A) -> Future<B, X> 
  {
    let (out_promise, out_future) = Promise::<B, X>::new();
    bind(self.state, move |result| match result {
      Ok(value) => attach(out_promise, callback(value)),
      Err(error) => out_promise.resolve_err(error),
    });
    out_future
  }

  pub fn catch<F: 'static, Y: 'static>(self, callback: F) -> Future<A, Y>
    where F: FnOnce(X) -> Future<A, Y> 
  {
    let (out_promise, out_future) = Promise::<A, Y>::new();
    bind(self.state, move |result| match result {
      Ok(value) => out_promise.resolve_ok(value),
      Err(error) => attach(out_promise, callback(error)),
    });
    out_future
  }

  pub fn map<F: 'static, B: 'static>(self, callback: F) -> Future<B, X>
    where F: FnOnce(A) -> B 
  {
    let (out_promise, out_future) = Promise::<B, X>::new();
    bind(self.state, move |result| match result {
      Ok(value) => out_promise.resolve_ok(callback(value)),
      Err(error) => out_promise.resolve_err(error),
    });
    out_future
  }

  pub fn map_err<F: 'static, Y: 'static>(self, callback: F) -> Future<A, Y>
    where F: FnOnce(X) -> Y
  {
    let (out_promise, out_future) = Promise::<A, Y>::new();
    bind(self.state, move |result| match result {
      Ok(value) => out_promise.resolve_ok(value),
      Err(error) => out_promise.resolve_err(callback(error)),
    });
    out_future
  }

  /// When the value of `self` becomes available, lends the `Ok` value to the
  /// callback, but does not otherwise modify the Future.
  pub fn tap<F: 'static>(self, callback: F) -> Future<A, X>
    where F: FnOnce(&A)
  {
    let (out_promise, out_future) = Promise::<A, X>::new();
    bind(self.state, move |result| {
      match result {
        Ok(ref value) => callback(value),
        Err(_) => (),
      }
      out_promise.resolve(result);
    });
    out_future
  }
}

#[cfg(test)]
mod test {
  use std::cell::{RefCell};
  use std::rc::{Rc};
  use super::{Promise, Future};

  #[test]
  fn test_resolve_first() {
    let out = Rc::new(RefCell::new(false));
    let out_clone = out.clone();

    let (promise, future) = Promise::<i32, char>::new();
    promise.resolve(Ok(1234));
    future.consume(move |result| {
      assert_eq!(result, Ok(1234));
      *out_clone.borrow_mut() = true;
    });

    assert_eq!(*out.borrow(), true);
  }

  #[test] 
  fn test_bind_first() {
    let out = Rc::new(RefCell::new(false));
    let out_clone = out.clone();

    let (promise, future) = Promise::<i32, char>::new();
    future.consume(move |result| {
      assert_eq!(result, Ok(1234));
      *out_clone.borrow_mut() = true;
    });

    assert_eq!(*out.borrow(), false);
    promise.resolve(Ok(1234));
    assert_eq!(*out.borrow(), true);
  }

  #[test]
  fn test_then() {
    let out = Rc::new(RefCell::new(false));
    let out_clone = out.clone();

    let (promise, future) = Promise::new();
    future.then(|value: i32| {
      assert_eq!(value, 1234);
      Future::new_ok('Z')
    }).then(|value: char| {
      assert_eq!(value, 'Z');
      Future::new_err(format!("kaboom"))
    }).then(|_: Vec<i32>| {
      panic!("Previous future should have failed")
    }).consume(move |result: Result<f32, _>| {
      assert_eq!(result, Err(format!("kaboom")));
      *out_clone.borrow_mut() = true;
    });

    assert_eq!(*out.borrow(), false);
    promise.resolve(Ok(1234));
    assert_eq!(*out.borrow(), true);
  }

  #[test]
  fn test_map() {
    let out = Rc::new(RefCell::new(false));
    let out_clone = out.clone();

    let (promise, future) = Promise::new();
    future.map(|value| {
      assert_eq!(value, 1234);
      'Z'
    }).map(|value| {
      assert_eq!(value, 'Z');
      vec![1, 2, 3]
    }).consume(move |result: Result<Vec<i32>, ()>| {
      assert_eq!(result, Ok(vec![1, 2, 3]));
      *out_clone.borrow_mut() = true;
    });

    assert_eq!(*out.borrow(), false);
    promise.resolve(Ok(1234));
    assert_eq!(*out.borrow(), true);
  }
}
