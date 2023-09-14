use tokio::sync::mpsc as tokio_mpsc;
use tokio::sync::oneshot as tokio_oneshot;

#[derive(Debug)]
pub struct NotifyReceiver<T: Send> {
    rx: tokio_mpsc::Receiver<(T, tokio_oneshot::Sender<()>)>,
}

#[derive(Debug)]
pub struct NotifyReceiverHandle<'a, T: Send> {
    receiver: &'a mut NotifyReceiver<T>,
    end_tx: Option<tokio_oneshot::Sender<()>>,
    value: T,
}

impl<T: Send> NotifyReceiver<T> {
    pub async fn recv(&mut self) -> Option<NotifyReceiverHandle<T>> {
        let (value, end_tx) = self.rx.recv().await?;

        Some(NotifyReceiverHandle {
            receiver: self,
            end_tx: Some(end_tx),
            value,
        })
    }
}

impl<T: Send> AsMut<T> for NotifyReceiverHandle<'_, T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

impl<T: Send> AsRef<T> for NotifyReceiverHandle<'_, T> {
    fn as_ref(&self) -> &T {
        &self.value
    }
}

impl<T: Send> std::ops::Deref for NotifyReceiverHandle<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: Send> std::ops::DerefMut for NotifyReceiverHandle<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T: Send> Drop for NotifyReceiverHandle<'_, T> {
    fn drop(&mut self) {
        let _ = self.end_tx.take().unwrap().send(());
    }
}

#[derive(Debug)]
pub struct NotifySender<T: Send> {
    tx: tokio_mpsc::Sender<(T, tokio_oneshot::Sender<()>)>,
}

impl<T: Send> NotifySender<T> {
    pub fn blocking_send(
        &self,
        value: T,
    ) -> Result<(), tokio_mpsc::error::SendError<(T, tokio_oneshot::Sender<()>)>> {
        let (end_tx, end_rx) = tokio_oneshot::channel();

        eprintln!("notify_sender: blocking_send");
        self.tx.blocking_send((value, end_tx))?;

        eprintln!("notify_sender: blocking_recv");
        end_rx.blocking_recv().unwrap();

        Ok(())
    }

    pub async fn send(
        &self,
        value: T,
    ) -> Result<(), tokio_mpsc::error::SendError<(T, tokio_oneshot::Sender<()>)>> {
        let (end_tx, end_rx) = tokio_oneshot::channel();
        self.tx.send((value, end_tx)).await?;

        end_rx.await.unwrap();

        Ok(())
    }
}

pub fn notify_channel<T: Send>() -> (NotifySender<T>, NotifyReceiver<T>) {
    let (tx, rx) = tokio_mpsc::channel(1);

    (NotifySender { tx }, NotifyReceiver { rx })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_notify_channel() {
        let (tx, mut rx) = notify_channel::<u32>();

        tokio::spawn(async move {
            eprintln!("waiting for value...");
            let res = rx.recv().await.unwrap();
            eprintln!("got value: {:?}", res);
            drop(res);
        });

        eprintln!("sending value and waiting for it to be received and processed...");
        tx.send(42).await.unwrap();

        eprintln!("finished test");
    }
}
