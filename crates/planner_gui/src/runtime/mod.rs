use std::fmt::Debug;
use std::future::Future;
use std::marker::PhantomData;
use futures::channel::mpsc;
use futures::{select, Sink, Stream, StreamExt};
use futures::stream::{BoxStream, FusedStream};
use cushy::value::{Destination, Dynamic};
use tracing::debug;

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
    S: Sink<M, Error = mpsc::SendError>
    + Unpin
    + Send
    + Clone
    + 'static,
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

        let message = self.sender.clone();
        let future =
            stream.map(move |message| {
                // FIXME Uncommenting this makes it work, but only because the additional time causes messages
                //       not to be missed.
                //debug!("stream message: {:?}", message);
                Ok(message)
            }).forward(message).map(|result| match result {
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
    pub async fn dispatch<T: Debug>(mut receiver: impl Stream<Item = T> + FusedStream + Unpin, message: Dynamic<T>) {
        loop {
            select! {
                received_message = receiver.select_next_some() => {
                    debug!("dispatcher received message: {:?}", received_message);
                    message.force_set(received_message);
                }
            }
        }
    }
}


pub fn boxed_stream<T, S>(stream: S) -> BoxStream<'static, T>
where
    S: futures::Stream<Item = T> + Send + 'static,
{
    futures::stream::StreamExt::boxed(stream)
}
