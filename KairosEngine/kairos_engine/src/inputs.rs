use std::collections::HashMap;

use winit::{event::KeyEvent, keyboard::PhysicalKey};

use crate::math::float2;

#[derive(Debug, Clone, Copy)]
pub enum Input {
    W,
    A,
    S,
    D,
    Mouse(float2),
    MouseLClick,
    MouseRClick,
}
impl Input {
    fn get_id(&self) -> usize {
        match self {
            Input::W => 0,
            Input::A => 1,
            Input::S => 2,
            Input::D => 3,
            Input::Mouse(_) => 4,
            Input::MouseLClick => 5,
            Input::MouseRClick => 6,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum InputState {
    None,
    Presse,
    Holding(f32),
    Release,
}

pub struct InputEngine {
    inputs_map: HashMap<PhysicalKey, Input>,
    input_indexs: HashMap<usize, usize>,
    states: Vec<InputState>,
}

impl InputEngine {
    pub fn new() -> Self {
        Self {
            inputs_map: HashMap::new(),
            input_indexs: HashMap::new(),
            states: Vec::new(),
        }
    }

    pub fn update(&mut self, delta_time: f32) {
        for state in &mut self.states {
            match state {
                InputState::None => {}
                InputState::Presse => {
                    *state = InputState::Holding(0.0);
                }
                InputState::Holding(timer) => {
                    *state = InputState::Holding(*timer + delta_time);
                }
                InputState::Release => {
                    *state = InputState::None;
                }
            }
        }
    }

    pub fn registe_input(&mut self, physics_input: PhysicalKey, input: Input) {
        self.inputs_map.insert(physics_input, input);
        let input_id = input.get_id();
        if !self.input_indexs.contains_key(&input_id) {
            let intpu_index = self.states.len();
            self.states.push(InputState::None);
            self.input_indexs.insert(input_id, intpu_index);
        }
    }

    pub fn unregiste_input(&mut self, physics_input: PhysicalKey) {
        match self.inputs_map.remove(&physics_input) {
            Some(input) => {
                let input_id = input.get_id();
                let Some(input_index) = self.input_indexs.remove(&input_id) else {
                    return;
                };
                self.states[input_index] = InputState::None;
            }
            None => {}
        }
    }

    pub fn update_keyboard_input(&mut self, event: KeyEvent) {
        let Some(&input) = self.inputs_map.get(&event.physical_key) else {
            return;
        };

        self.update_input_state(input, event.state);
    }

    fn update_input_state(&mut self, input: Input, state: winit::event::ElementState) {
        let input_state = &mut self.states[input.get_id()];
        match state {
            winit::event::ElementState::Pressed => match input_state {
                InputState::None => *input_state = InputState::Presse,
                InputState::Presse => {}
                InputState::Holding(_) => {}
                InputState::Release => *input_state = InputState::Presse,
            },
            winit::event::ElementState::Released => match input_state {
                InputState::None => *input_state = InputState::Release,
                InputState::Presse => *input_state = InputState::Release,
                InputState::Holding(_) => *input_state = InputState::Release,
                InputState::Release => {}
            },
        }
    }
}
