use std::fmt::Debug;
use std::future::Future;
use std::marker::PhantomData;

use futures::channel::mpsc;
use futures::stream::{BoxStream, FusedStream};
use futures::{Sink, Stream, StreamExt, select};
use tracing::{error, trace};

#[derive(Debug)]
pub struct Executor;

impl Executor {
    pub fn new() -> Result<Self, futures::io::Error> {
        Ok(Self)
    }

    pub fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        let _ = async_std::task::spawn(future);
    }
}

pub struct RunTime<S, M> {
    executor: Executor,
    sender: S,
    _message: PhantomData<M>,
}

impl<S, M> RunTime<S, M>
where
    S: Sink<M, Error = mpsc::SendError> + Unpin + Send + Clone + 'static,
    M: Send + Debug + 'static,
{
    pub fn new(executor: Executor, sender: S) -> Self {
        Self {
            executor,
            sender,
            _message: PhantomData::default(),
        }
    }

    pub fn run(&mut self, stream: BoxStream<'static, M>) {
        use futures::{FutureExt, StreamExt};

        let sender = self.sender.clone();
        let future = stream
            .map(move |message| {
                //trace!("stream message: {:?}", message);
                Ok(message)
            })
            .forward(sender)
            .map(|result| match result {
                Ok(()) => (),
                Err(error) => {
                    println!("Stream unable to complete, cause: {error}");
                }
            });

        self.executor.spawn(future);
    }
}

pub struct MessageDispatcher {}

impl MessageDispatcher {
    pub async fn dispatch<T: Send + Debug + 'static>(
        mut receiver: impl Stream<Item = T> + FusedStream + Unpin,
        sender: std::sync::mpsc::Sender<T>,
    ) {
        loop {
            select! {
                received_message = receiver.select_next_some() => {
                    trace!("dispatcher. task message: {:?}", &received_message);
                    match sender.send(received_message) {

                        Ok(_) => trace!("dispatch. completed"),
                        Err(message) => {
                            error!("dispatch. error dispatching task message: {:?}", message);
                        }
                    };
                }
            }
        }
    }
}

pub fn boxed_stream<T, S>(stream: S) -> BoxStream<'static, T>
where
    S: Stream<Item = T> + Send + 'static,
{
    StreamExt::boxed(stream)
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use futures::stream::FusedStream;
    use futures::{Stream, StreamExt, select};
    use tracing::{error, trace};

    use crate::runtime::{Executor, RunTime};
    use crate::task;
    use crate::task::Task;

    #[test]
    pub fn wrapped_messages() {
        // given
        let task: Task<Result<InnerMessage, TestError>> = Task::done(Ok(InnerMessage::Inner1));

        let task: Task<OuterMessage> = task.map(|result| match result {
            Ok(message) => OuterMessage::Wrapped(message),
            Err(error) => OuterMessage::Error(error),
        });

        // when
        let messages = run(task);

        // then
        dump_collected_messages(&messages);
        assert_eq!(messages, vec![OuterMessage::Wrapped(InnerMessage::Inner1)]);
    }

    #[test]
    pub fn batched_messages() {
        // given

        // this is the same as what is returned by a call to a crux service's `update()` method
        let results: Vec<Task<Result<InnerMessage, TestError>>> =
            vec![Task::done(Ok(InnerMessage::Inner1)), Task::done(Err(TestError::Error1))];

        let task: Task<Result<InnerMessage, TestError>> = Task::batch(results);

        let task: Task<OuterMessage> = task.map(|result| match result {
            Ok(message) => OuterMessage::Wrapped(message),
            Err(error) => OuterMessage::Error(error),
        });

        // when
        let messages = run(task);

        // then
        dump_collected_messages(&messages);
        assert_eq!(messages, vec![
            OuterMessage::Wrapped(InnerMessage::Inner1),
            OuterMessage::Error(TestError::Error1),
        ]);
    }

    #[test]
    pub fn batched_messages_with_chain() {
        // given

        // this is the same as what is returned by a call to a crux service's `update()` method
        let results: Vec<Task<Result<InnerMessage, TestError>>> =
            vec![Task::done(Ok(InnerMessage::Inner1)), Task::done(Err(TestError::Error1))];

        let task: Task<Result<InnerMessage, TestError>> = Task::batch(results);

        let task = task.chain(Task::done(Ok(InnerMessage::Inner2)));

        let task: Task<OuterMessage> = task.map(|result| match result {
            Ok(message) => OuterMessage::Wrapped(message),
            Err(error) => OuterMessage::Error(error),
        });

        // when
        let messages = run(task);

        // then
        dump_collected_messages(&messages);
        assert_eq!(messages, vec![
            OuterMessage::Wrapped(InnerMessage::Inner1),
            OuterMessage::Error(TestError::Error1),
            // Note that the chained message is present, even though there was a preceding error
            OuterMessage::Wrapped(InnerMessage::Inner2),
        ]);
    }

    #[test]
    pub fn batched_messages_with_and_then() {
        // given

        // this is the same as what is returned by a call to a crux service's `update()` method
        let results: Vec<Task<Result<InnerMessage, TestError>>> =
            vec![Task::done(Ok(InnerMessage::Inner1)), Task::done(Err(TestError::Error1))];

        let task: Task<Result<InnerMessage, TestError>> = Task::batch(results);

        let messages_to_assert = Arc::new(Mutex::new(vec![]));

        let task = task.and_then({
            let messages_to_assert = messages_to_assert.clone();
            move |last_inner_message| {
                println!("last inner message: {:?}", &last_inner_message);
                messages_to_assert
                    .lock()
                    .unwrap()
                    .push(last_inner_message);

                Task::done(Ok(InnerMessage::Inner2))
            }
        });

        let task: Task<OuterMessage> = task.map(|result| match result {
            Ok(message) => OuterMessage::Wrapped(message),
            Err(error) => OuterMessage::Error(error),
        });

        // when
        let messages = run(task);

        // then
        dump_collected_messages(&messages);
        assert_eq!(messages, vec![
            // Note that the inner messages are gone!
            OuterMessage::Wrapped(InnerMessage::Inner2),
        ]);

        // and
        let messages_to_assert = Arc::into_inner(messages_to_assert)
            .unwrap()
            .into_inner()
            .unwrap();
        assert_eq!(messages_to_assert, vec![InnerMessage::Inner1,]);
    }

    #[test]
    pub fn batched_messages_with_map() {
        // given

        // this is the same as what is returned by a call to a crux service's `update()` method
        let results: Vec<Task<Result<InnerMessage, TestError>>> =
            vec![Task::done(Ok(InnerMessage::Inner1)), Task::done(Err(TestError::Error1))];

        let task: Task<Result<InnerMessage, TestError>> = Task::batch(results);

        let results_to_assert = Arc::new(Mutex::new(vec![]));

        let task = task.map({
            let results_to_assert = results_to_assert.clone();
            move |result| {
                println!("result passed to `.map`: {:?}", &result);
                results_to_assert
                    .lock()
                    .unwrap()
                    .push(result.clone());

                Ok(InnerMessage::Inner2)
            }
        });

        let task: Task<OuterMessage> = task.map(|result| match result {
            Ok(message) => OuterMessage::Wrapped(message),
            Err(error) => OuterMessage::Error(error),
        });

        // when
        let messages = run(task);

        // then
        dump_collected_messages(&messages);
        assert_eq!(messages, vec![
            // both the inner messages were replaced with a different message
            OuterMessage::Wrapped(InnerMessage::Inner2),
            OuterMessage::Wrapped(InnerMessage::Inner2),
        ]);

        // and
        let results_to_assert = Arc::into_inner(results_to_assert)
            .unwrap()
            .into_inner()
            .unwrap();
        assert_eq!(results_to_assert, [Ok(InnerMessage::Inner1), Err(TestError::Error1),]);
    }

    #[test]
    pub fn batched_messages_with_then() {
        // given

        // this is the same as what is returned by a call to a crux service's `update()` method
        let results: Vec<Task<Result<InnerMessage, TestError>>> = vec![
            Task::done(Ok(InnerMessage::Inner1)),
            Task::done(Err(TestError::Error1)),
            Task::done(Ok(InnerMessage::Inner2)),
        ];

        let task: Task<Result<InnerMessage, TestError>> = Task::batch(results);

        let results_to_assert = Arc::new(Mutex::new(vec![]));

        let task = task.then({
            let results_to_assert = results_to_assert.clone();
            move |result| {
                println!("result passed to `.map`: {:?}", &result);
                results_to_assert
                    .lock()
                    .unwrap()
                    .push(result.clone());

                Task::done(Ok(InnerMessage::Inner3))
            }
        });

        let task: Task<OuterMessage> = task.map(|result| match result {
            Ok(message) => OuterMessage::Wrapped(message),
            Err(error) => OuterMessage::Error(error),
        });

        // when
        let messages = run(task);

        // then
        dump_collected_messages(&messages);
        assert_eq!(messages, vec![
            // note that all the inner tasks were mapped, including the errors
            OuterMessage::Wrapped(InnerMessage::Inner3),
            OuterMessage::Wrapped(InnerMessage::Inner3),
            OuterMessage::Wrapped(InnerMessage::Inner3),
        ]);

        // and
        let results_to_assert = Arc::into_inner(results_to_assert)
            .unwrap()
            .into_inner()
            .unwrap();
        assert_eq!(results_to_assert, [
            Ok(InnerMessage::Inner1),
            Err(TestError::Error1),
            Ok(InnerMessage::Inner2),
        ]);
    }

    fn dump_collected_messages(messages: &Vec<OuterMessage>) {
        for message in messages.iter() {
            println!("message: {:?}", message)
        }
    }

    fn run(final_task: Task<OuterMessage>) -> Vec<OuterMessage> {
        let (app_message_sender, app_message_receiver) = std::sync::mpsc::channel::<OuterMessage>();

        let (sender, receiver) = futures::channel::mpsc::unbounded();

        let executor = Executor::new().expect("should be able to create an executor");
        executor.spawn(TestMessageDispatcher::dispatch(receiver, app_message_sender.clone()));
        let mut runtime = RunTime::new(executor, sender.clone());

        if let Some(stream) = task::into_stream(final_task) {
            runtime.run(stream);
        }

        // FIXME this may cause flakey tests if the system running the tests is slow/loaded.
        //       ideally we collect all received messages until the `collect_messages` thread is told to stop...

        let handle = thread::spawn(|| collect_messages(app_message_receiver));
        thread::sleep(Duration::from_millis(250));

        let messages = handle.join().unwrap();
        messages
    }

    fn collect_messages<T>(receiver: std::sync::mpsc::Receiver<T>) -> Vec<T> {
        receiver.try_iter().collect()
    }

    pub struct TestMessageDispatcher {}

    impl TestMessageDispatcher {
        pub async fn dispatch<T: Send + Debug + 'static>(
            mut receiver: impl Stream<Item = T> + FusedStream + Unpin,
            sender: std::sync::mpsc::Sender<T>,
        ) {
            loop {
                select! {
                    received_message = receiver.select_next_some() => {
                        trace!("dispatcher. task message: {:?}", &received_message);
                        match sender.send(received_message) {

                            Ok(_) => trace!("dispatch. completed"),
                            Err(message) => {
                                error!("dispatch. error dispatching task message: {:?}", message);
                            }
                        };
                    }
                }
            }
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    enum InnerMessage {
        Inner1,
        Inner2,
        Inner3,
    }

    #[derive(Debug, PartialEq, Clone)]
    enum OuterMessage {
        Wrapped(InnerMessage),
        Error(TestError),
    }

    #[derive(Debug, PartialEq, Clone)]
    #[allow(dead_code)]
    enum TestError {
        Error1,
    }
}
