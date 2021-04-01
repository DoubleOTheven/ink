// Copyright 2018-2021 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::{
    ext::{
        self,
        Engine,
        Error,
    },
    test_api,
};

const BUFFER_SIZE: usize = 1024;

fn get_buffer() -> [u8; 1024] {
    [0; BUFFER_SIZE]
}

#[test]
fn store_load_clear() {
    let mut engine = Engine::new();
    let key = &[0x42; 32][..];
    let output = &mut &mut get_buffer()[..];
    let res = engine.get_storage(key, output);
    assert_eq!(res, Err(Error::KeyNotFound));

    engine.set_storage(&key, &[0x05_u8; 5]);
    let res = engine.get_storage(&key, output);
    assert_eq!(res, Ok(()),);
    assert_eq!(output[..5], [0x05; 5]);

    engine.clear_storage(&key);
    let res = engine.get_storage(key, output);
    assert_eq!(res, Err(Error::KeyNotFound));
}

#[test]
fn setting_getting_balance() {
    // given
    let mut engine = Engine::new();
    let account_id = vec![1; 32];
    let balance = 1337;
    engine.set_callee(account_id.clone());
    engine.set_balance(account_id, balance);

    // when
    let mut output = get_buffer();
    engine.balance(&mut &mut output[..]);

    // then
    let output = <u128 as scale::Decode>::decode(&mut &output[..16])
        .expect("decoding balance failed");
    assert_eq!(output, balance);
}

#[test]
fn setting_getting_caller() {
    // given
    let mut engine = Engine::new();
    let account_id = vec![1; 32];

    // when
    engine.set_caller(account_id.clone());

    // then
    let mut output = get_buffer();
    engine.caller(&mut &mut output[..]);
    assert_eq!(&output[..account_id.len()], &account_id);
}

#[test]
fn address() {
    // given
    let mut engine = Engine::new();
    let account_id = vec![1; 32];
    engine.set_callee(account_id.clone());

    // when
    let mut output = get_buffer();
    engine.address(&mut &mut output[..]);

    // then
    assert_eq!(&output[..account_id.len()], &account_id);
}

#[test]
fn transfer() {
    // given
    let mut engine = Engine::new();
    let alice = vec![1; 32];
    let bob = vec![2; 32];
    engine.set_callee(alice.clone());
    engine.set_balance(alice.clone(), 1337);

    // when
    let val = scale::Encode::encode(&337u128);
    assert_eq!(engine.transfer(&bob, &val), Ok(()));

    // then
    assert_eq!(engine.get_balance(alice), Ok(1000));
    assert_eq!(engine.get_balance(bob), Ok(337));
}

#[test]
fn printlns() {
    let mut engine = Engine::new();
    engine.println("foobar");
    let mut recorded = engine.get_recorded_printlns();
    assert_eq!(recorded.next(), Some("foobar".into()));
    assert_eq!(recorded.next(), None);
}

#[test]
fn events() {
    // given
    let mut engine = Engine::new();
    let topics_count: scale::Compact<u32> = scale::Compact(2u32);
    let mut enc_topics_count = scale::Encode::encode(&topics_count);
    let topic1 = vec![12u8, 13];
    let topic2 = vec![14u8, 15];
    let data = &vec![21, 22, 23];

    // when
    let mut enc_topics_info: Vec<u8> = Vec::new();
    enc_topics_info.append(&mut enc_topics_count);
    enc_topics_info.append(&mut topic1.clone());
    enc_topics_info.append(&mut topic2.clone());
    engine.deposit_event(&enc_topics_info, data);

    // then
    let events = engine.get_emitted_events();
    assert_eq!(events.len(), 1);

    let event = events.get(0).expect("event must exist");
    assert_eq!(event.topics.len(), 2);
    assert_eq!(
        event.topics.get(0).expect("first topic must exist"),
        &topic1
    );
    assert_eq!(
        event.topics.get(1).expect("second topic must exist"),
        &topic2
    );
    assert_eq!(&event.data, data);
}

#[test]
fn value_transferred() {
    // given
    let mut engine = Engine::new();
    let value = 1337;
    engine.set_value_transferred(value);

    // when
    let output = &mut &mut get_buffer()[..];
    engine.value_transferred(output);

    // then
    let output = <u128 as scale::Decode>::decode(&mut &output[..16])
        .expect("decoding value transferred failed");
    assert_eq!(output, value);
}
