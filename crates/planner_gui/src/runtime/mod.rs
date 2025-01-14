use std::future::Future;
use std::marker::PhantomData;
use std::thread::sleep;
use std::time::Duration;
use futures::channel::mpsc;
use futures::{select, Sink, Stream, StreamExt};
use futures::stream::{BoxStream, FusedStream};
use cushy::value::{Destination, Dynamic};

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
    M: Send + 'static,
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
            stream.map(|item|{
                // FIXME making the thread sleep fixes missing stream items, no idea why!
                //       without this items in the stream are lost
                //       additionally in release mode, or when debug logging is disabled, using a shorter period
                //       e.g. `from_micros(1)` does not work around the issue.
                //       In the current code base, the issue can be seen in the UI, commenting out
                //       the `sleep` call will cause the app to miss the processing of the 
                //       ProjectMessage::RequestView(ProjectViewRequest::ProjectOverview))
                //       See `Project::update` where it handles `ProjectMessage::Loaded` and calls
                //       `.update(Event::RequestOverviewView {})`.  The core receives the event, but
                //       the task generated via `app_core::process_effect`'s `Effect::ViewRenderer`
                //       handler is lost.
                sleep(Duration::from_millis(1));
                
                Ok(item)
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
    pub async fn dispatch<T>(mut receiver: impl Stream<Item = T> + FusedStream + Unpin, message: Dynamic<T>) {
        loop {
            select! {
                received_message = receiver.select_next_some() => {
                    //println!("dispatcher received message: {:?}", received_message);
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
