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

        let task: Task<OuterMessage> = task.map(|result|{
            match result {
                Ok(message) => OuterMessage::Wrapped(message),
                Err(error) => OuterMessage::Error(error) ,
            }
        });

        // when
        let messages = run(task);
        
        // then
        dump_collected_messages(&messages);
        assert_eq!(messages, vec![OuterMessage::Wrapped(InnerMessage::Inner1)])
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

        let handle = thread::spawn(|| {
            collect_messages(app_message_receiver)
        });

        thread::sleep(Duration::from_millis(100));

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

    #[derive(Debug, PartialEq)]
    enum InnerMessage {
        Inner1
    }

    #[derive(Debug, PartialEq)]
    enum OuterMessage {
        Wrapped(InnerMessage),
        Error(TestError)
    }

    #[derive(Debug, PartialEq)]
    #[allow(dead_code)]
    enum TestError {
        Error1
    }

}