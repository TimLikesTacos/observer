use async_trait::async_trait;

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Weak};
use tokio::sync::Mutex;

/// Trait for an observable object that performs notifications asyncronously.
pub trait ObservableAsync<E> {
    fn register_observer_mut(
        &mut self,
        observer: Arc<Mutex<dyn ObserverAsyncMut<E>>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;

    fn register_observer(
        &mut self,
        observer: Arc<dyn ObserverAsync<E>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;

    fn notify_observers<'a>(
        &'a self,
        event: &'a E,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    where
        E: Send;
}

/// Trait for an observer that can be notified asyncronously.
#[async_trait]
pub trait ObserverAsyncMut<E>: Send + Sync {
    async fn notify_mut(&mut self, event: &E);
}

/// Trait for an observer that can be notified asyncronously.
#[async_trait]
pub trait ObserverAsync<E>: Send + Sync {
    async fn notify(&self, event: &E);
}

enum Mutability<E> {
    Mutable(Weak<Mutex<dyn ObserverAsyncMut<E>>>),
    Immutable(Weak<dyn ObserverAsync<E>>),
}
pub struct DispatcherAsync<E> {
    observers: Mutex<Vec<Mutability<E>>>,
}

impl<E> ObservableAsync<E> for DispatcherAsync<E>
where
    E: Send + Sync,
{
    fn register_observer_mut(
        &mut self,
        observer: Arc<Mutex<dyn ObserverAsyncMut<E>>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            let mut lock = self.observers.lock().await;
            lock.push(Mutability::Mutable(Arc::downgrade(&observer)));
        })
    }

    fn register_observer(
        &mut self,
        observer: Arc<dyn ObserverAsync<E>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            let mut lock = self.observers.lock().await;
            lock.push(Mutability::Immutable(Arc::downgrade(&observer)));
        })
    }

    fn notify_observers<'a>(&'a self, event: &'a E) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    where
        E: Send + Sync,
    {
        Box::pin(async move {
            let mut futures = Vec::new();
            let mut lock = self.observers.lock().await;
            // This removes any observers that have been dropped (fail to upgrade)
            lock.retain(|observer| {
                match observer {
                    Mutability::Mutable(observer) => {
                        if let Some(ob) = observer.upgrade() {
                            // Don't handle the mutex here, as it will be dropped. Need to pass it into the future.
                            let future = handle_notify_mut(ob, event);
                            futures.push(future);

                            true
                        } else {
                            false
                        }
                    }
                    Mutability::Immutable(observer) => {
                        if let Some(ob) = observer.upgrade() {
                            futures.push(Box::pin(async move { ob.notify(event).await }));

                            true
                        } else {
                            false
                        }
                    }
                }
            });
            futures::future::join_all(futures).await;
        })
    }
}

/// Used for moving the mutex into the future.
fn handle_notify_mut<E>(
    observer: Arc<Mutex<dyn ObserverAsyncMut<E>>>,
    event: &E,
) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>
where
    E: Send + Sync,
{
    Box::pin(async move {
        let mut lock = observer.lock().await;
        lock.notify_mut(event).await;
    })
}

impl<E> DispatcherAsync<E> {
    pub fn new() -> Self {
        DispatcherAsync {
            observers: Mutex::new(Vec::new()),
        }
    }

    pub async fn num_oberservers(&self) -> usize {
        self.observers.lock().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Observer1(i32);

    #[async_trait]
    impl ObserverAsyncMut<i32> for Observer1 {
        async fn notify_mut(&mut self, event: &i32) {
            self.0 += event;
            println!("Observer1 call event: {}", event);
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            println!("Observer1: {}, result {}", event, self.0);
        }
    }

    struct Observer2(i32);

    #[async_trait]
    impl ObserverAsyncMut<i32> for Observer2 {
        async fn notify_mut(&mut self, event: &i32) {
            self.0 *= event;
            println!("Observer2 call event: {}", event);
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            println!("Observer2: {}, result {}", event, self.0);
        }
    }

    #[tokio::test]
    async fn test_dispatcher() {
        let mut dispatcher = DispatcherAsync::new();
        let observer1 = Arc::new(Mutex::new(Observer1(0)));
        let observer2 = Arc::new(Mutex::new(Observer2(1)));

        dispatcher.register_observer_mut(observer1.clone()).await;
        dispatcher.register_observer_mut(observer2.clone()).await;

        assert_eq!(dispatcher.num_oberservers().await, 2);

        dispatcher.notify_observers(&1).await;

        assert_eq!(observer1.lock().await.0, 1);
        assert_eq!(observer2.lock().await.0, 1);

        dispatcher.notify_observers(&2).await;

        assert_eq!(dispatcher.num_oberservers().await, 2);

        assert_eq!(observer1.lock().await.0, 3);
        assert_eq!(observer2.lock().await.0, 2);

        dispatcher.notify_observers(&10).await;

        assert_eq!(dispatcher.num_oberservers().await, 2);

        assert_eq!(observer1.lock().await.0, 13);
        assert_eq!(observer2.lock().await.0, 20);
    }

    #[tokio::test]
    async fn test_dispatcher_bulk_notify() {
        let mut dispatcher = DispatcherAsync::new();
        let observer1 = Arc::new(Mutex::new(Observer1(0)));
        let observer2 = Arc::new(Mutex::new(Observer2(1)));

        dispatcher.register_observer_mut(observer1.clone()).await;
        dispatcher.register_observer_mut(observer2.clone()).await;

        assert_eq!(dispatcher.num_oberservers().await, 2);

        let mut futures = vec![];

        let f = dispatcher.notify_observers(&1);
        let f2 = dispatcher.notify_observers(&2);
        let f3 = dispatcher.notify_observers(&10);
        futures.push(f);
        futures.push(f2);
        futures.push(f3);

        futures::future::join_all(futures).await;

        assert_eq!(observer1.lock().await.0, 13);
        assert_eq!(observer2.lock().await.0, 20);
    }

    #[tokio::test]
    async fn scoped_observer() {
        let mut dispatcher = DispatcherAsync::new();
        let observer1 = Arc::new(Mutex::new(Observer1(0)));
        let observer2 = Arc::new(Mutex::new(Observer2(3)));

        dispatcher.register_observer_mut(observer1.clone()).await;
        dispatcher.register_observer_mut(observer2.clone()).await;

        assert_eq!(dispatcher.num_oberservers().await, 2);

        dispatcher.notify_observers(&2).await;

        assert_eq!(observer1.lock().await.0, 2);
        assert_eq!(observer2.lock().await.0, 6);

        // Drop showing that the dispatcher does not own the observers
        drop(observer1);

        assert_eq!(dispatcher.num_oberservers().await, 2);

        dispatcher.notify_observers(&2).await;

        assert_eq!(dispatcher.num_oberservers().await, 1);

        assert_eq!(observer2.lock().await.0, 12);
    }

    #[tokio::test]
    async fn scoped_dispatcher_ownership_demo() {
        let mut dispatcher = DispatcherAsync::new();
        let observer1 = Arc::new(Mutex::new(Observer1(0)));
        let observer2 = Arc::new(Mutex::new(Observer2(3)));

        dispatcher.register_observer_mut(observer1.clone()).await;
        dispatcher.register_observer_mut(observer2.clone()).await;

        assert_eq!(dispatcher.num_oberservers().await, 2);

        dispatcher.notify_observers(&2).await;

        // Dropping dispatcher to show that ownership of the observers is not tied to the dispatcher
        drop(dispatcher);

        assert_eq!(observer1.lock().await.0, 2);
        assert_eq!(observer2.lock().await.0, 6);
    }
}
