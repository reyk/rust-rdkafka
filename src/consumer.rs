extern crate libc;
extern crate librdkafka_sys as rdkafka;
extern crate std;

use std::ffi::CString;
use std::str;

use error::{KafkaError, IsError};
use util::cstr_to_owned;

use config::KafkaConfig;

pub use config::CreateConsumer;

pub struct KafkaConsumer {
    client_n: *mut rdkafka::rd_kafka_t,
}

impl CreateConsumer<KafkaConsumer, KafkaError> for KafkaConfig {
    fn create_consumer(&self) -> Result<KafkaConsumer, KafkaError> {
        let errstr = [0i8; 1024];
        let rd_config = try!(self.create_kafka_config());
        let client_n = unsafe {
            rdkafka::rd_kafka_new(rdkafka::rd_kafka_type_t::RD_KAFKA_CONSUMER,
                                  rd_config,
                                  errstr.as_ptr() as *mut i8,
                                  errstr.len())
        };
        if client_n.is_null() {
            return Err(KafkaError::ConsumerCreationError(cstr_to_owned(&errstr)));
        }
        unsafe { rdkafka::rd_kafka_poll_set_consumer(client_n) };
        Ok(KafkaConsumer { client_n: client_n })
    }
}

impl KafkaConsumer {
    pub fn broker_add(&mut self, brokers: &str) -> i32 {
        let brokers = CString::new(brokers).unwrap();
        unsafe { rdkafka::rd_kafka_brokers_add(self.client_n, brokers.as_ptr()) }
    }

    pub fn subscribe(&mut self, topic_name: &str) -> Result<(), KafkaError> {
        let topic_name_c = CString::new(topic_name).unwrap();
        let ret_code = unsafe {
            let tp_list = rdkafka::rd_kafka_topic_partition_list_new(1);
            rdkafka::rd_kafka_topic_partition_list_add(tp_list, topic_name_c.as_ptr(), 0);
            rdkafka::rd_kafka_subscribe(self.client_n, tp_list)
        };
        if ret_code.is_error() {
            Err(KafkaError::SubscriptionError(topic_name.to_string()))
        } else {
            Ok(())
        }
    }

    pub fn poll(&self, timeout_ms: i32) -> Result<Option<Message>, KafkaError> {
        let message_n = unsafe { rdkafka::rd_kafka_consumer_poll(self.client_n, timeout_ms) };
        if message_n.is_null() {
            return Ok(None);
        }
        let error = unsafe { (*message_n).err };
        if error.is_error() {
            return Err(KafkaError::MessageConsumptionError(error));
        }
        let payload = unsafe {
            if (*message_n).payload.is_null() {
                None
            } else {
                Some(std::slice::from_raw_parts::<u8>((*message_n).payload as *const u8, (*message_n).len))
            }
        };
        let key = unsafe {
            if (*message_n).key.is_null() {
                None
            } else {
                Some(std::slice::from_raw_parts::<u8>((*message_n).key as *const u8, (*message_n).key_len))
            }
        };
        let message = Message {
            payload: payload,
            key: key,
            partition: unsafe { (*message_n).partition },
            offset: unsafe { (*message_n).offset },
            message_n: message_n,
        };
        Ok(Some(message))
    }
}

impl Drop for KafkaConsumer {
    fn drop(&mut self) {
        unsafe { rdkafka::rd_kafka_consumer_close(self.client_n) };
        unsafe { rdkafka::rd_kafka_destroy(self.client_n) };
        unsafe { rdkafka::rd_kafka_wait_destroyed(1000) };
    }
}

#[derive(Debug)]
pub struct Message<'a> {
    pub payload: Option<&'a [u8]>,
    pub key: Option<&'a [u8]>,
    pub partition: i32,
    pub offset: i64,
    pub message_n: *mut rdkafka::rd_kafka_message_s,
}

impl<'a> Drop for Message<'a> {
    fn drop(&mut self) {
        unsafe { rdkafka::rd_kafka_message_destroy(self.message_n) };
    }
}
