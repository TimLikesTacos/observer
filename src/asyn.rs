use async_trait::async_trait;

use std::sync::{Arc, Mutex, Weak};

pub trait Observable<E> {
    fn register_observer(&mut self, event: Arc<Mutex<dyn Observer<E>>>);

    async fn notify_observers(&mut self, event: &E);
}

#[async_trait]
pub trait Observer<E> {
    async fn notify(&mut self, event: &E);
}

pub struct Dispatcher<E> {
    observers: Vec<Weak<Mutex<dyn Observer<E>>>>,
}

impl<E> Observable<E> for Dispatcher<E> {
    fn register_observer(&mut self, observer: Arc<Mutex<dyn Observer<E>>>) {
        self.observers.push(Arc::downgrade(&observer));
    }

    async fn notify_observers(&mut self, event: &E) {
        let mut futures = Vec::new();
        // This removes any observers that have been dropped (fail to upgrade)
        self.observers.retain(|observer| {
            if let Some(observer) = observer.upgrade() {
                // Don't handle the mutex here, as it will be dropped. Need to pass it into the future.
                let future = handle_notify(observer, event);
                futures.push(future);

                true
            } else {
                false
            }
        });
        futures::future::join_all(futures).await;
    }
}

/// Used for moving the mutex into the future.
async fn handle_notify<E>(observer: Arc<Mutex<dyn Observer<E>>>, event: &E) {
    let mut lock = (*observer).lock().expect("Mutex failure");
    lock.notify(&event).await;
}

impl<E> Dispatcher<E> {
    pub fn new() -> Self {
        Dispatcher {
            observers: Vec::new(),
        }
    }

    pub fn num_oberservers(&self) -> usize {
        self.observers.len()
    }
}

#[cfg(test)]
#[cfg(not(feature = "async"))]
mod tests {
    use super::*;

    struct Observer1(i32);
    impl Observer<i32> for Observer1 {
        fn notify(&mut self, event: &i32) {
            self.0 += event;
            println!("Observer1: {}, result {}", event, self.0);
        }
    }

    struct Observer2(i32);
    impl Observer<i32> for Observer2 {
        fn notify(&mut self, event: &i32) {
            self.0 *= event;
            println!("Observer2: {}, result {}", event, self.0);
        }
    }

    #[test]
    fn test_dispatcher() {
        let mut dispatcher = Dispatcher::new();
        let observer1 = Arc::new(RefCell::new(Observer1(0)));
        let observer2 = Arc::new(RefCell::new(Observer2(1)));

        dispatcher.register_observer(observer1.clone());
        dispatcher.register_observer(observer2.clone());

        assert_eq!(dispatcher.num_oberservers(), 2);

        dispatcher.notify_observers(&1);

        assert_eq!(observer1.borrow().0, 1);
        assert_eq!(observer2.borrow().0, 1);

        dispatcher.notify_observers(&2);

        assert_eq!(dispatcher.num_oberservers(), 2);

        assert_eq!(observer1.borrow().0, 3);
        assert_eq!(observer2.borrow().0, 2);
    }

    #[test]
    fn scoped_observer() {
        let mut dispatcher = Dispatcher::new();
        let observer1 = Arc::new(RefCell::new(Observer1(0)));
        let observer2 = Arc::new(RefCell::new(Observer2(3)));

        dispatcher.register_observer(observer1.clone());
        dispatcher.register_observer(observer2.clone());

        assert_eq!(dispatcher.num_oberservers(), 2);

        dispatcher.notify_observers(&2);

        assert_eq!(observer1.borrow().0, 2);
        assert_eq!(observer2.borrow().0, 6);

        // Drop showing that the dispatcher does not own the observers
        drop(observer1);

        assert_eq!(dispatcher.num_oberservers(), 2);

        dispatcher.notify_observers(&2);

        assert_eq!(dispatcher.num_oberservers(), 1);

        assert_eq!(observer2.borrow().0, 12);
    }

    #[test]
    fn scoped_dispatcher_ownership_demo() {
        let mut dispatcher = Dispatcher::new();
        let observer1 = Arc::new(RefCell::new(Observer1(0)));
        let observer2 = Arc::new(RefCell::new(Observer2(3)));

        dispatcher.register_observer(observer1.clone());
        dispatcher.register_observer(observer2.clone());

        assert_eq!(dispatcher.num_oberservers(), 2);

        dispatcher.notify_observers(&2);

        // Dropping dispatcher to show that ownership of the observers is not tied to the dispatcher
        drop(dispatcher);

        assert_eq!(observer1.borrow().0, 2);
        assert_eq!(observer2.borrow().0, 6);
    }
}
