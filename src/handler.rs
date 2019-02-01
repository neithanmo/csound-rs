#![allow(non_camel_case_types, non_upper_case_globals, non_snake_case)]

use rtaudio::{CS_AudioDevice, RT_AudioParams};
use enums::{ChannelData, MessageType, Status};
use csound::CallbackHandler;
use callbacks::*;

#[derive(Default)]
pub struct Callbacks<'a> {
    pub message_cb:         Option<Box<FnMut(MessageType, &str) + 'a>>,
    pub audio_dev_list_cb:  Option<Box<FnMut(CS_AudioDevice) + 'a>>,
    pub play_open_cb:       Option<Box<FnMut(&RT_AudioParams)->Status + 'a>>,
    pub rec_open_cb:        Option<Box<FnMut(&RT_AudioParams)->Status + 'a>>,
    pub rt_play_cb:         Option<Box<FnMut(&[f64]) + 'a>>,
    pub rt_rec_cb:          Option<Box<FnMut(&mut[f64])->usize + 'a>>,
    pub sense_event_cb:     Option<Box<FnMut() + 'a>>,
    //pub keyboard_cb:        Option<Box<FnMut(i32) + 'a>>, // TODO this callback doesn't work at the
    //csound side
    pub rt_close_cb:        Option<Box<FnMut() + 'a>>,
    pub cscore_cb:          Option<Box<FnMut() + 'a>>,
    pub input_channel_cb:   Option<Box<FnMut(&str)->ChannelData + 'a >>,
    pub output_channel_cb:  Option<Box<FnMut(&str, ChannelData) + 'a >>,
    pub file_open_cb:       Option<Box<FnMut(&FileInfo) + 'a >>,
    pub midi_in_open_cb:    Option<Box<FnMut(&str) + 'a >>,
    pub midi_out_open_cb:   Option<Box<FnMut(&str) + 'a >>,
    pub midi_read_cb:       Option<Box<FnMut(&[u8])->usize + 'a>>,
    pub midi_write_cb:      Option<Box<FnMut(&mut[u8])->usize + 'a>>,
    pub midi_in_close_cb:   Option<Box<FnMut() + 'a>>,
    pub midi_out_close_cb:  Option<Box<FnMut() + 'a>>,
}


/// Trait for the various callbacks used by csound to invoke user functions.
///
/// This trait represent all callbacks in the csound API, some of then are not supported yet,
/// because of their undefine behavior.
pub trait Handler{

    fn message_cb(&mut self, message_type: MessageType, _message: &str);

    fn audio_dev_list_cb(&mut self, dev: CS_AudioDevice);

    fn play_open_cb(&mut self, _rt_audio: &RT_AudioParams) -> Status;

    fn rec_open_cb(&mut self, rt_audio: &RT_AudioParams) -> Status;

    fn rt_play_cb(&mut self, buffer: &[f64]);

    fn rt_rec_cb(&mut self, buffer: &mut[f64]) -> usize;

    fn rt_close_cb(&mut self);

    fn sense_event_cb(&mut self);

    //fn keyboard_cb(&mut self, value: i32);

    fn cscore_cb(&mut self);

    fn input_channel_cb( &mut self, name:&str ) -> ChannelData;

    fn output_channel_cb(&mut self, name:&str, channel: ChannelData);

    fn file_open_cb(&mut self, info:&FileInfo);

    fn midi_in_open_cb(&mut self, devName: &str);

    fn midi_out_open_cb(&mut self, devName: &str);

    fn midi_read_cb(&mut self, buffer: &[u8]) -> usize;

    fn midi_write_cb(&mut self, buffer: &mut[u8])->usize;

    fn midi_in_close_cb(&mut self);

    fn midi_out_close_cb(&mut self);

}

impl Handler for CallbackHandler {

    fn message_cb(&mut self, message_type: MessageType, message: &str){
        match self.callbacks.message_cb.as_mut() {
            Some(fun) => fun(message_type, message),
            None => drop(message),
        }
    }

    fn file_open_cb(&mut self, info:&FileInfo){
        if let Some(fun) = self.callbacks.file_open_cb.as_mut() {
            fun(info);
        }
    }

    fn audio_dev_list_cb(&mut self, dev: CS_AudioDevice) {
        if let Some(fun) = self.callbacks.audio_dev_list_cb.as_mut() {
            fun(dev);
        }
    }

    fn rt_play_cb(&mut self, buff: &[f64]){
        if let Some(fun) = self.callbacks.rt_play_cb.as_mut() {
            fun(buff);
        }
    }

    fn rt_rec_cb(&mut self, buff: &mut[f64]) -> usize{
        if let Some(fun) = self.callbacks.rt_rec_cb.as_mut() {
            return fun(buff)
        }
        0
    }

    fn play_open_cb(&mut self, params: &RT_AudioParams) -> Status{
        if let Some(fun) = self.callbacks.play_open_cb.as_mut() {
            return fun(params)
        }
        Status::CS_ERROR
    }

    fn rec_open_cb(&mut self, rec: &RT_AudioParams) -> Status{
        if let Some(fun) = self.callbacks.rec_open_cb.as_mut() {
            return fun(rec)
        }
        Status::CS_ERROR
    }

    fn sense_event_cb(&mut self){
        if let Some(fun) = self.callbacks.sense_event_cb.as_mut() {
            fun();
        }
    }

    /*
     *fn keyboard_cb(&mut self, value: i32){
     *    match self.callbacks.keyboard_cb.as_mut() {
     *        Some(fun) => fun(value),
     *        None => {},
     *    }
     *}
     */

    fn rt_close_cb(&mut self){
        if let Some(fun) = self.callbacks.rt_close_cb.as_mut() {
            fun();
        }
    }

    fn cscore_cb(&mut self){
        if let Some(fun) =  self.callbacks.cscore_cb.as_mut() {
            fun();
        }
    }

    fn input_channel_cb(&mut self, name: &str) -> ChannelData {
        if let Some(fun) =self.callbacks.input_channel_cb.as_mut() {
            return fun(name)
        }
        ChannelData::CS_UNKNOWN_CHANNEL
    }

    fn output_channel_cb(&mut self, name: &str, channel: ChannelData){
        if let Some(fun) =  self.callbacks.output_channel_cb.as_mut() {
            fun(name, channel);
        }
    }



    fn midi_in_open_cb(&mut self, devName: &str){
        if let Some(fun) = self.callbacks.midi_in_open_cb.as_mut() {
            fun(devName);
        }
    }

    fn midi_out_open_cb(&mut self, devName: &str){
        if let Some(fun) =  self.callbacks.midi_out_open_cb.as_mut() {
            fun(devName);
        }
    }

    fn midi_read_cb(&mut self, buff: &[u8])->usize{
        if let Some(fun) =  self.callbacks.midi_read_cb.as_mut() {
            return fun(buff)
        }
        0
    }

    fn midi_write_cb(&mut self, buff: &mut[u8])->usize{
        if let Some(fun) =  self.callbacks.midi_write_cb.as_mut() {
            return fun(buff)
        }
        0
    }

    fn midi_in_close_cb(&mut self){
        if let Some(fun) = self.callbacks.midi_in_close_cb.as_mut() {
            fun();
        }
    }

    fn midi_out_close_cb(&mut self){
        if let Some(fun) = self.callbacks.midi_out_close_cb.as_mut() {
            fun();
        }
    }
}
