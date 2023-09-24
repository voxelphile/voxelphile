use std::collections::VecDeque;
use std::iter;
use std::mem;

use crate::world::block::Block;

pub fn encode(blocks: &[Block]) -> Vec<u8> {
    assert!(blocks.len() >= 1);
    let mut curr = blocks[0];
    let mut count = 1u32;
    let mut cursor = 1;
    let mut data = Vec::with_capacity(mem::size_of::<Block>() * blocks.len() * 3);
    loop {
        if curr == blocks[cursor] && cursor + 1 < blocks.len() {
            count += 1;
            cursor += 1;
            continue;
        }
        if curr != blocks[cursor] || cursor + 1 >= blocks.len() {
            data.extend_from_slice(&count.to_be_bytes());
            data.extend_from_slice(
                &unsafe { mem::transmute::<_, u16>(curr) }.to_be_bytes(),
            );
            if cursor + 1 < blocks.len() {
                count = 1;
                curr = blocks[cursor];
                cursor += 1;
                continue;
            }
        }
        break data;
    }
}

pub fn decode(mut bytes: Vec<u8>) -> Vec<Block> {
    assert!(bytes.len() % 6 == 0);
    let mut blocks = Vec::with_capacity(bytes.len() / mem::size_of::<Block>() / 3);
    loop {
        let (y,x) = (bytes.pop().unwrap(), bytes.pop().unwrap());
        let (d,c,b,a) = (
            bytes.pop().unwrap(),
            bytes.pop().unwrap(),
            bytes.pop().unwrap(),
            bytes.pop().unwrap(),
        );
        let block = unsafe { mem::transmute::<_, Block>(u16::from_be_bytes([x, y])) };
        let num = u32::from_be_bytes([a, b, c, d]);
        blocks.extend(iter::repeat(block).take(num as usize));
        if bytes.len() == 0 {
            blocks.reverse();
            break blocks;
        }
    }
}
