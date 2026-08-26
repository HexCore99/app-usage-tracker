# Learn

## `unsafe` Block

- Windows API functions are written in C style and Rust cannot verify that these functions are safe. that's why i need to write them in unsafe block

## [0u16,512] - Array Creation

- Create an Array with:
  - 512 elements
  - each element is u16
  - initialize everything with zero

## Slice the buffer `&buffer[..length as usize]`

- `buffer[..6]` -> take first 6 elements
- `length as usize` -> converts i32 to usize
