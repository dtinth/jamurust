pub struct JitterBuffer<T> {
    size: usize,
    frames: Vec<Frame<T>>,
    latest_sequence_number: u8,
}

struct Frame<T> {
    sequence_number: u8,
    payload: Option<T>,
}

impl<T> JitterBuffer<T> {
    pub fn new(size: usize) -> JitterBuffer<T> {
        JitterBuffer {
            size: size,
            frames: Vec::with_capacity(size),
            latest_sequence_number: 0,
        }
    }
    pub fn put_in(&mut self, frame: T, sequence_number: u8) -> Option<T> {
        if self.frames.len() == self.size {
            // Pick the oldest frame and return it
            let latest_sequence_number = self.latest_sequence_number;
            let mut oldest_frame = self
                .frames
                .iter_mut()
                .max_by_key(|f| Self::distance(latest_sequence_number, f.sequence_number))
                .unwrap();

            let payload = oldest_frame.payload.take();
            // Put a new frame in its place
            self.latest_sequence_number = sequence_number;
            oldest_frame.sequence_number = sequence_number;
            oldest_frame.payload = Some(frame);

            payload
        } else {
            // Add the frame to the buffer
            self.frames.push(Frame {
                sequence_number: sequence_number,
                payload: Some(frame),
            });
            None
        }
    }
    fn distance(latest_sequence_number: u8, sequence_number: u8) -> i16 {
        let mut diff = (latest_sequence_number as i16) - (sequence_number as i16);
        if diff < -128 {
            diff += 256;
        }
        if diff > 128 {
            diff -= 256;
        }
        diff
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jitter_buffer_works() {
        let mut buffer = JitterBuffer::new(3);
        assert_eq!(buffer.put_in("A", 20), None);
        assert_eq!(buffer.put_in("B", 21), None);
        assert_eq!(buffer.put_in("C", 22), None);
        assert_eq!(buffer.put_in("D", 23), Some("A"));
        assert_eq!(buffer.put_in("E", 24), Some("B"));
        assert_eq!(buffer.put_in("F", 25), Some("C"));
    }

    #[test]
    fn jitter_buffer_can_handle_jitter() {
        let mut buffer = JitterBuffer::new(3);
        assert_eq!(buffer.put_in("C", 22), None);
        assert_eq!(buffer.put_in("B", 21), None);
        assert_eq!(buffer.put_in("A", 20), None);
        assert_eq!(buffer.put_in("E", 24), Some("A"));
        assert_eq!(buffer.put_in("F", 25), Some("B"));
        assert_eq!(buffer.put_in("D", 23), Some("C"));
    }

    #[test]
    fn jitter_buffer_works_at_u8_boundary() {
        let mut buffer = JitterBuffer::new(3);
        assert_eq!(buffer.put_in("A", 253), None);
        assert_eq!(buffer.put_in("D", 0), None);
        assert_eq!(buffer.put_in("C", 255), None);
        assert_eq!(buffer.put_in("B", 254), Some("A"));
        assert_eq!(buffer.put_in("F", 2), Some("B"));
        assert_eq!(buffer.put_in("E", 1), Some("C"));
    }
}
