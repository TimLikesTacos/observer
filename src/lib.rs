use std::cell::RefCell;
use std::sync::{Arc, Weak};

pub trait Observable<E> {
    fn register_observer(&mut self, event: Arc<RefCell<dyn Observer<E>>>);
    fn notify_observers(&mut self, event: &E);
}

pub trait Observer<E> {
    fn notify(&mut self, event: &E);
}

pub struct Dispatcher<E> {
    observers: Vec<Weak<RefCell<dyn Observer<E>>>>,
}

impl<E> Observable<E> for Dispatcher<E> {
    fn register_observer(&mut self, observer: Arc<RefCell<dyn Observer<E>>>) {
        self.observers.push(Arc::downgrade(&observer));
    }
    fn notify_observers(&mut self, event: &E) {
        // This removes any observers that have been dropped
        self.observers.retain(|observer| {
            if let Some(observer) = observer.upgrade() {
                observer.borrow_mut().notify(event);
                true
            } else {
                false
            }
        });
    }
}

impl Dispatcher<i32> {
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
}
