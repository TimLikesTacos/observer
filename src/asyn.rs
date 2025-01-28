use async_trait::async_trait;

use std::sync::{Arc, Weak};

use tokio::sync::Mutex;

pub trait ObservableAsync<E> {
    fn register_observer(&mut self, event: Arc<Mutex<dyn ObserverAsync<E> + Send>>);

    fn notify_observers(&self, event: &E) -> impl std::future::Future<Output = Vec<()>> + Send
    where
        E: Send;
}

#[async_trait]
pub trait ObserverAsync<E> {
    async fn notify(&mut self, event: &E);
}

pub struct DispatcherAsync<E> {
    observers: Vec<Weak<Mutex<dyn ObserverAsync<E> + Send>>>,
}

impl<E> ObservableAsync<E> for DispatcherAsync<E>
where
    E: Send + Sync,
{
    fn register_observer(&mut self, observer: Arc<Mutex<dyn ObserverAsync<E> + Send>>) {
        self.observers.push(Arc::downgrade(&observer));
    }

    // fn notify_observers(&mut self, event: &E) -> impl std::future::Future<Output = Vec<()>> + Send
    // where
    //     E: Send + Sync,
    // {
    //     let mut futures = Vec::new();
    //     // This removes any observers that have been dropped (fail to upgrade)
    //     self.observers.retain(|observer| {
    //         if let Some(observer) = observer.upgrade() {
    //             // Don't handle the mutex here, as it will be dropped. Need to pass it into the future.
    //             let future = handle_notify(observer, event);
    //             futures.push(future);

    //             true
    //         } else {
    //             false
    //         }
    //     });
    //     futures::future::join_all(futures)
    // }

    fn notify_observers(&self, event: &E) -> impl std::future::Future<Output = Vec<()>> + Send
    where
        E: Send + Sync,
    {
        let mut futures = Vec::new();
        // This removes any observers that have been dropped (fail to upgrade)
        let v: Vec<_> = self
            .observers
            .iter()
            .map(|observer| {
                if let Some(observer) = observer.upgrade() {
                    // Don't handle the mutex here, as it will be dropped. Need to pass it into the future.
                    let future = handle_notify(observer, event);
                    futures.push(future);

                    true
                } else {
                    false
                }
            })
            .collect();
        futures::future::join_all(futures)
    }
}

/// Used for moving the mutex into the future.
async fn handle_notify<E>(observer: Arc<Mutex<dyn ObserverAsync<E> + Send>>, event: &E) {
    let mut lock = (*observer).lock().await;
    lock.notify(&event).await;
}

impl<E> DispatcherAsync<E> {
    pub fn new() -> Self {
        DispatcherAsync {
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
    use tokio::test;
    struct Observer1(i32);

    #[async_trait]
    impl ObserverAsync<i32> for Observer1 {
        async fn notify(&mut self, event: &i32) {
            self.0 += event;
            println!("Observer1 call event: {}", event);
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            println!("Observer1: {}, result {}", event, self.0);
        }
    }

    struct Observer2(i32);

    #[async_trait]
    impl ObserverAsync<i32> for Observer2 {
        async fn notify(&mut self, event: &i32) {
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

        dispatcher.register_observer(observer1.clone());
        dispatcher.register_observer(observer2.clone());

        assert_eq!(dispatcher.num_oberservers(), 2);

        dispatcher.notify_observers(&1).await;

        assert_eq!(observer1.lock().await.0, 1);
        assert_eq!(observer2.lock().await.0, 1);

        dispatcher.notify_observers(&2).await;

        assert_eq!(dispatcher.num_oberservers(), 2);

        assert_eq!(observer1.lock().await.0, 3);
        assert_eq!(observer2.lock().await.0, 2);

        dispatcher.notify_observers(&10).await;

        assert_eq!(dispatcher.num_oberservers(), 2);

        assert_eq!(observer1.lock().await.0, 13);
        assert_eq!(observer2.lock().await.0, 20);
    }

    #[tokio::test]
    async fn test_dispatcher_bulk_notify() {
        let mut dispatcher = DispatcherAsync::new();
        let observer1 = Arc::new(Mutex::new(Observer1(0)));
        let observer2 = Arc::new(Mutex::new(Observer2(1)));

        dispatcher.register_observer(observer1.clone());
        dispatcher.register_observer(observer2.clone());

        assert_eq!(dispatcher.num_oberservers(), 2);

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

        dispatcher.register_observer(observer1.clone());
        dispatcher.register_observer(observer2.clone());

        assert_eq!(dispatcher.num_oberservers(), 2);

        dispatcher.notify_observers(&2).await;

        assert_eq!(observer1.lock().await.0, 2);
        assert_eq!(observer2.lock().await.0, 6);

        // Drop showing that the dispatcher does not own the observers
        drop(observer1);

        assert_eq!(dispatcher.num_oberservers(), 2);

        dispatcher.notify_observers(&2).await;

        assert_eq!(dispatcher.num_oberservers(), 1);

        assert_eq!(observer2.lock().await.0, 12);
    }

    #[tokio::test]
    async fn scoped_dispatcher_ownership_demo() {
        let mut dispatcher = DispatcherAsync::new();
        let observer1 = Arc::new(Mutex::new(Observer1(0)));
        let observer2 = Arc::new(Mutex::new(Observer2(3)));

        dispatcher.register_observer(observer1.clone());
        dispatcher.register_observer(observer2.clone());

        assert_eq!(dispatcher.num_oberservers(), 2);

        dispatcher.notify_observers(&2).await;

        // Dropping dispatcher to show that ownership of the observers is not tied to the dispatcher
        drop(dispatcher);

        assert_eq!(observer1.lock().await.0, 2);
        assert_eq!(observer2.lock().await.0, 6);
    }
}
