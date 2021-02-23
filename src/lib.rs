use core::fmt::Formatter;
use std::{sync::Arc, time::Duration};

use tokio::{
    io::{AsyncReadExt},
    prelude::*,
    sync::{Mutex},
    time::delay_for,
};
use tokio_serial::{Serial, SerialPort};
use tracing::{debug, instrument, trace_span};
use tracing_futures::Instrument;


const BUFFER_SIZE: usize = 1024;

/// Driver that speaks to an underlying serial device that understands COBS-encoded postcard packets
/// This driver is safe to use concurrently and exposes an async interface.
///
/// This system is a request-response mechanism.
pub struct Driver {
    /// Underlying serial device to communicate with
    connection: Interface,
}

impl std::fmt::Debug for Driver {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // tokio_serial::Serial doesn't implement debug or fmt...
        // and thanks to this being behind an async mutex i can't ask it for information
        // in a sync context without probably borking something.

        write!(f, "Drivetrain(<Serial>)")
    }
}

pub type Interface = Arc<Mutex<Serial>>;

impl Driver where Driver: Sync + Send
{
    /// Creates a new driver from an underlying `connection`, taking ownership of it.
    pub fn new(connection: Serial) -> Self {
        let connection = Arc::new(Mutex::new(connection));
        Driver { connection }
    }
    /// Executes a request against the underlying hardware, and returns its result
    /// TODO: example
    #[instrument]
    pub async fn do_hardware_action(
        &self,
        request: rover_postcards::Request,
    ) -> Result<rover_postcards::Response, tokio_serial::Error> {
        let mut tx_buffer = bytes::BytesMut::new();
        tx_buffer.resize(BUFFER_SIZE, 0xFF);

        // Encode payload...
        let payload = postcard::to_slice_cobs(&request, &mut tx_buffer).unwrap();
        debug!("payload request is {:?}", request);
        debug!("committing payload {:?}", payload);

        {
            // critical section
            let mut connection_guard = self.connection.lock().await;
            // Commit to device.
            connection_guard
                .write_all(&payload)
                .instrument(trace_span!("writing to device"))
                .await?;
            debug!("done writing to device...");
        }

        // Alloc buffer.
        let mut rx_buffer = bytes::BytesMut::new();
        let read_length = self
            .read_serial_until(&mut rx_buffer, 0x00)
            .instrument(trace_span!("reading from device..."))
            .await?;
        debug!(read_length = read_length, "read from device");

        // Decode response.
        let response: rover_postcards::Response =
            postcard::from_bytes_cobs(&mut rx_buffer[..read_length]).unwrap();
        Ok(response)
    }
    /// A buffered read abstraction that reads from the device until a specified byte is read.
    #[instrument]
    pub(crate) async fn read_serial_until(
        &self,
        buffer: &mut bytes::BytesMut,
        until: u8,
    ) -> Result<usize, tokio_serial::Error> {
        let mut done = false;
        let mut len: usize = 0;
        // entire function is a critical section
        let mut connection_guard = self.connection.lock().await;

        while !done {
            while connection_guard.bytes_to_read()? == 0 {
                delay_for(Duration::from_millis(15)).await;
            }
            let bytes_to_take = connection_guard.bytes_to_read()? as u64;
            debug!(taking = bytes_to_take, "taking bytes");
            buffer.resize((buffer.len() as u64 + bytes_to_take) as usize, 0xFF);

            let read_bytes = connection_guard.read(&mut buffer[len..]).await?;
            debug!(read = read_bytes, "bytes read");
            len += read_bytes;
            // read length is one beyond
            done = buffer[len - 1] == until;
            debug!("buffer now reads :: {:?}", buffer)
        }
        Ok(len)
    }
}

#[cfg(test)]
mod tests {
    use tokio::prelude::*;
    use tokio_serial;
    use tracing_futures::Instrument;
    use tracing::debug_span;
    mod logging;

    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[instrument]
    pub fn setup() {
        logging::setup_logger();
    }

    #[tokio::test]
    async fn test_read_serial_until() {
        setup();
        // Asserts that the `read_serial_until` actually works using fake serial objects

        // Spawn a faked connected pair of Serial objects for this test.
        let (rx, mut tx) = tokio_serial::Serial::pair().unwrap();

        let iface = Driver::new(rx);

        // Spawn a async worker that transmits the requisite data as two packets.
        let worker = tokio::task::spawn(
            async move {
                // Write response in two packets.
                tx.write_all(&[1, 2, 42, 1]).await.unwrap();
                // Wait well beyond what the unit under test waits for...
                tokio::time::delay_for(tokio::time::Duration::from_millis(50)).await;
                // Send the second packet.
                tx.write_all(&[1, 1, 1, 0]).await.unwrap();
            }
                .instrument(trace_span!("worker task")),
        );
        // Initialize a buffer to recv data into
        let mut rx_buffer = bytes::BytesMut::with_capacity(1024);
        // Invoke unit under test
        let future = iface
            .read_serial_until(&mut rx_buffer, 0x00)
            .instrument(debug_span!("test"));
        // Schedule it for running with a reasonable timeout.
        // Note: Timeout necessary to prevent potential infinite runtime.
        let read_bytes = tokio::time::timeout(tokio::time::Duration::from_secs(1), future)
            .await
            .unwrap()
            .unwrap();
        // Assert it did the thing...
        assert_eq!(read_bytes, 8);
        const EXPECTED: &[u8] = &[1, 2, 42, 1, 1, 1, 1, 0];
        assert_eq!(rx_buffer, bytes::Bytes::from(EXPECTED));
        // And join the worker..
        worker.await.unwrap();
    }
}
