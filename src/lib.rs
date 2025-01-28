//! This is a simple implementation of the Observer pattern in Rust.
//! The Observer pattern is a behavioral design pattern that defines a one-to-many dependency between objects so that when one object changes state, all its dependents are notified and updated automatically.
//! The pattern is implemented using a Dispatcher struct that holds a list of observers. The Dispatcher struct implements the Observable trait which defines the methods to register and notify observers.
//! The Observer trait defines the method that is called when the observable notifies the observer.
//! The Dispatcher struct is generic over the type of event that is passed to the observers.
//! The Dispatcher can register any observer provided it implements the Observer trait and the event type is the same as the Dispatcher's event type.
//! Mismatching Oberservers types is possible provided they use the same event type.
//! Ownership of the observers is not tied to the Dispatcher. This means that the Dispatcher does not own the observers and the observers can be dropped without affecting the Dispatcher.
#[cfg(feature = "async")]
pub mod asyn;

#[cfg(feature = "async")]
pub use crate::asyn::*;

/// Reexport async trait as it's needed to implement the ObserverAsync trait
#[cfg(feature = "async")]
pub use async_trait::async_trait;

pub use crate::sync::*;

mod sync {

    use std::cell::RefCell;
    use std::rc::{Rc, Weak};

    enum Mutability<E> {
        Mutable(Weak<RefCell<dyn ObserverMut<E>>>),
        Immutable(Weak<dyn Observer<E>>),
    }

    pub trait Observable<E> {
        fn register_observer(&mut self, event: Rc<dyn Observer<E>>);
        fn register_observer_mut(&mut self, event: Rc<RefCell<dyn ObserverMut<E>>>);
        fn notify_observers(&mut self, event: &E);
    }

    pub trait Observer<E> {
        fn notify(&self, event: &E);
    }

    pub trait ObserverMut<E> {
        fn notify_mut(&mut self, event: &E);
    }

    pub struct Dispatcher<E> {
        observers: Vec<Mutability<E>>,
    }

    impl<E> Observable<E> for Dispatcher<E> {
        fn register_observer(&mut self, observer: Rc<dyn Observer<E>>) {
            self.observers
                .push(Mutability::Immutable(Rc::downgrade(&observer)));
        }

        fn register_observer_mut(&mut self, observer: Rc<RefCell<dyn ObserverMut<E>>>) {
            self.observers
                .push(Mutability::Mutable(Rc::downgrade(&observer)));
        }

        fn notify_observers(&mut self, event: &E) {
            // This removes any observers that have been dropped (fail to upgrade)
            self.observers.retain(|observer| match observer {
                Mutability::Mutable(observer) => {
                    if let Some(observer) = observer.upgrade() {
                        observer.borrow_mut().notify_mut(event);
                        return true;
                    } else {
                        false
                    }
                }
                Mutability::Immutable(observer) => {
                    if let Some(observer) = observer.upgrade() {
                        observer.notify(event);
                        return true;
                    } else {
                        false
                    }
                }
            });
        }
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
    mod tests {
        use super::*;

        mod immutable {
            use super::*;
            struct Observer1(i32);
            impl Observer<i32> for Observer1 {
                fn notify(&self, event: &i32) {
                    println!("Observer1: {}, result {}", event, self.0);
                }
            }

            struct Observer2(i32);
            impl Observer<i32> for Observer2 {
                fn notify(&self, event: &i32) {
                    println!("Observer2: {}, result {}", event, self.0);
                }
            }

            #[test]
            fn test_dispatcher() {
                let mut dispatcher = Dispatcher::new();
                let observer1 = Rc::new(Observer1(0));
                let observer2 = Rc::new(Observer2(1));

                dispatcher.register_observer(observer1.clone());
                dispatcher.register_observer(observer2.clone());

                assert_eq!(dispatcher.num_oberservers(), 2);

                dispatcher.notify_observers(&1);

                dispatcher.notify_observers(&2);

                assert_eq!(dispatcher.num_oberservers(), 2);
            }

            #[test]
            fn scoped_observer() {
                let mut dispatcher = Dispatcher::new();
                let observer1 = Rc::new(Observer1(0));
                let observer2 = Rc::new(Observer2(1));

                dispatcher.register_observer(observer1.clone());
                dispatcher.register_observer(observer2.clone());

                assert_eq!(dispatcher.num_oberservers(), 2);

                dispatcher.notify_observers(&1);
                assert_eq!(dispatcher.num_oberservers(), 2);

                // Drop showing that the dispatcher does not own the observers
                drop(observer1);

                assert_eq!(dispatcher.num_oberservers(), 2);

                dispatcher.notify_observers(&2);

                assert_eq!(dispatcher.num_oberservers(), 1);
            }
        }
        mod mutable {
            use super::*;
            struct Observer1(i32);
            impl ObserverMut<i32> for Observer1 {
                fn notify_mut(&mut self, event: &i32) {
                    self.0 += event;
                    println!("Observer1: {}, result {}", event, self.0);
                }
            }

            struct Observer2(i32);
            impl ObserverMut<i32> for Observer2 {
                fn notify_mut(&mut self, event: &i32) {
                    self.0 *= event;
                    println!("Observer2: {}, result {}", event, self.0);
                }
            }

            #[test]
            fn test_dispatcher() {
                let mut dispatcher = Dispatcher::new();
                let observer1 = Rc::new(RefCell::new(Observer1(0)));
                let observer2 = Rc::new(RefCell::new(Observer2(1)));

                dispatcher.register_observer_mut(observer1.clone());
                dispatcher.register_observer_mut(observer2.clone());

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
                let observer1 = Rc::new(RefCell::new(Observer1(0)));
                let observer2 = Rc::new(RefCell::new(Observer2(3)));

                dispatcher.register_observer_mut(observer1.clone());
                dispatcher.register_observer_mut(observer2.clone());

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
                let observer1 = Rc::new(RefCell::new(Observer1(0)));
                let observer2 = Rc::new(RefCell::new(Observer2(3)));

                dispatcher.register_observer_mut(observer1.clone());
                dispatcher.register_observer_mut(observer2.clone());

                assert_eq!(dispatcher.num_oberservers(), 2);

                dispatcher.notify_observers(&2);

                // Dropping dispatcher to show that ownership of the observers is not tied to the dispatcher
                drop(dispatcher);

                assert_eq!(observer1.borrow().0, 2);
                assert_eq!(observer2.borrow().0, 6);
            }
        }

        mod mutable_and_immutable {
            use super::*;
            struct MutOb(i32);
            impl ObserverMut<i32> for MutOb {
                fn notify_mut(&mut self, event: &i32) {
                    self.0 += event;
                    println!("MutableOb: {}, result {}", event, self.0);
                }
            }

            struct ImmutOb(i32);
            impl Observer<i32> for ImmutOb {
                fn notify(&self, event: &i32) {
                    println!("ImmutableOb: {}, result {}", event, self.0);
                }
            }
            #[test]
            fn test_dispatcher() {
                let mut dispatcher = Dispatcher::new();
                let mut_observer = Rc::new(RefCell::new(MutOb(0)));
                let immut_observer = Rc::new(ImmutOb(1));

                dispatcher.register_observer_mut(mut_observer.clone());
                dispatcher.register_observer(immut_observer.clone());

                assert_eq!(dispatcher.num_oberservers(), 2);

                dispatcher.notify_observers(&1);

                assert_eq!(mut_observer.borrow().0, 1);

                dispatcher.notify_observers(&2);

                assert_eq!(dispatcher.num_oberservers(), 2);

                assert_eq!(mut_observer.borrow().0, 3);

                drop(immut_observer);
                dispatcher.notify_observers(&2);
                assert_eq!(mut_observer.borrow().0, 5);
                assert_eq!(dispatcher.num_oberservers(), 1);
            }
        }
    }
}
