//! StateBuf assembly, diff processing, and element parsing.

use std::collections::HashMap;
use std::io::{Cursor, Read, Seek, SeekFrom};

use super::DecodeError;
use super::game_messages::GamePlayStatusMessage;
use super::opcodes;

// ============================================================
// Checksum
// ============================================================

/// Compute the rolling checksum over a byte buffer.
/// Algorithm: seed = 826366246, then for each byte: checksum = (checksum << 1) + byte
pub fn compute_checksum(data: &[u8]) -> i32 {
    let mut checksum = opcodes::CHECKSUM_SEED;
    for &b in data {
        checksum = (checksum << 1).wrapping_add(b as i32);
    }
    checksum
}

// ============================================================
// Diff algorithm (ApplyDiffs2)
// ============================================================

/// Apply a diff to an old state buffer, producing a new state buffer.
/// Implements the ApplyDiffs2 algorithm from PROTOCOL_RESEARCH.md.
pub fn apply_diffs(old_state: &[u8], diff_data: &[u8]) -> Result<Vec<u8>, DecodeError> {
    let mut output = Vec::new();
    let mut diff_cursor = 0usize;
    let mut old_cursor = 0usize;

    while diff_cursor < diff_data.len() {
        let leading = diff_data[diff_cursor];
        diff_cursor += 1;

        if leading == 0x00 {
            // Copy: uint16 count + int16 seek
            if diff_cursor + 4 > diff_data.len() {
                return Err(DecodeError::DiffOutOfBounds {
                    context: "copy opcode truncated",
                });
            }
            let count =
                u16::from_le_bytes([diff_data[diff_cursor], diff_data[diff_cursor + 1]]) as usize;
            let seek =
                i16::from_le_bytes([diff_data[diff_cursor + 2], diff_data[diff_cursor + 3]]);
            diff_cursor += 4;

            let new_pos = (old_cursor as isize) + (seek as isize);
            if new_pos < 0 || new_pos as usize + count > old_state.len() {
                return Err(DecodeError::DiffOutOfBounds {
                    context: "copy seek/count out of bounds",
                });
            }
            old_cursor = new_pos as usize;
            output.extend_from_slice(&old_state[old_cursor..old_cursor + count]);
            old_cursor += count;
        } else if leading & 0x80 != 0 {
            let low7 = (leading & 0x7F) as usize;
            if low7 == 0 {
                // 0x80 with low 7 bits == 0
                if diff_cursor >= diff_data.len() {
                    return Err(DecodeError::DiffOutOfBounds {
                        context: "0x80 opcode: missing next byte",
                    });
                }
                let next = diff_data[diff_cursor];
                diff_cursor += 1;

                if next == 0 {
                    // Long literal: 3-byte LE count, then that many literal bytes
                    if diff_cursor + 3 > diff_data.len() {
                        return Err(DecodeError::DiffOutOfBounds {
                            context: "long literal: count truncated",
                        });
                    }
                    let count = diff_data[diff_cursor] as usize
                        | ((diff_data[diff_cursor + 1] as usize) << 8)
                        | ((diff_data[diff_cursor + 2] as usize) << 16);
                    diff_cursor += 3;

                    if diff_cursor + count > diff_data.len() {
                        return Err(DecodeError::DiffOutOfBounds {
                            context: "long literal: data truncated",
                        });
                    }
                    output.extend_from_slice(&diff_data[diff_cursor..diff_cursor + count]);
                    diff_cursor += count;
                } else {
                    // Medium copy: next byte = count, int16 seek
                    let count = next as usize;
                    if diff_cursor + 2 > diff_data.len() {
                        return Err(DecodeError::DiffOutOfBounds {
                            context: "medium copy: seek truncated",
                        });
                    }
                    let seek = i16::from_le_bytes([
                        diff_data[diff_cursor],
                        diff_data[diff_cursor + 1],
                    ]);
                    diff_cursor += 2;

                    let new_pos = (old_cursor as isize) + (seek as isize);
                    if new_pos < 0 || new_pos as usize + count > old_state.len() {
                        return Err(DecodeError::DiffOutOfBounds {
                            context: "medium copy seek/count out of bounds",
                        });
                    }
                    old_cursor = new_pos as usize;
                    output.extend_from_slice(&old_state[old_cursor..old_cursor + count]);
                    old_cursor += count;
                }
            } else {
                // Short copy: count = low 7 bits, sbyte seek
                let count = low7;
                if diff_cursor >= diff_data.len() {
                    return Err(DecodeError::DiffOutOfBounds {
                        context: "short copy: seek truncated",
                    });
                }
                let seek = diff_data[diff_cursor] as i8;
                diff_cursor += 1;

                let new_pos = (old_cursor as isize) + (seek as isize);
                if new_pos < 0 || new_pos as usize + count > old_state.len() {
                    return Err(DecodeError::DiffOutOfBounds {
                        context: "short copy seek/count out of bounds",
                    });
                }
                old_cursor = new_pos as usize;
                output.extend_from_slice(&old_state[old_cursor..old_cursor + count]);
                old_cursor += count;
            }
        } else {
            // 0x01..0x7F: literal
            let count = leading as usize;
            if diff_cursor + count > diff_data.len() {
                return Err(DecodeError::DiffOutOfBounds {
                    context: "literal: data truncated",
                });
            }
            output.extend_from_slice(&diff_data[diff_cursor..diff_cursor + count]);
            diff_cursor += count;
        }
    }

    Ok(output)
}

// ============================================================
// StateBuf Processor
// ============================================================

/// Manages StateBuf assembly across chunked messages and diff application.
///
/// MTGO interleaves state updates for multiple views (e.g., both players'
/// perspectives) in the same stream. Each view has its own diff chain.
/// The processor uses `last_state_checksum` to route each diff to the
/// correct base state. When checksums collide (common due to the weak
/// rolling checksum), it tries each candidate until one produces a result
/// matching the expected checksum.
pub struct StateBufProcessor {
    assembly_buffer: Vec<u8>,
    /// Recent states keyed by checksum. Multiple states may share a checksum.
    state_cache: Vec<(i32, Vec<u8>)>,
}

/// Maximum number of states to keep in the cache.
const STATE_CACHE_MAX: usize = 16;

impl StateBufProcessor {
    pub fn new() -> Self {
        Self {
            assembly_buffer: Vec::new(),
            state_cache: Vec::new(),
        }
    }

    /// Store a state in the cache, evicting old entries if needed.
    fn cache_state(&mut self, state: &[u8]) {
        let checksum = compute_checksum(state);
        // Remove any existing entry with the same checksum and size
        self.state_cache
            .retain(|(cs, s)| !(*cs == checksum && s.len() == state.len()));
        if self.state_cache.len() >= STATE_CACHE_MAX {
            self.state_cache.remove(0);
        }
        self.state_cache.push((checksum, state.to_vec()));
    }

    /// Find a base state for a diff by trying candidates matching last_state_checksum.
    fn find_diff_base(
        &self,
        diff_data: &[u8],
        last_state_checksum: i32,
        expected_checksum: i32,
    ) -> Option<Vec<u8>> {
        // Try states matching last_state_checksum first
        for (cs, base) in self.state_cache.iter().rev() {
            if *cs != last_state_checksum {
                continue;
            }
            if let Ok(result) = apply_diffs(base, diff_data) {
                let result_cs = compute_checksum(&result);
                if result_cs == expected_checksum {
                    return Some(result);
                }
            }
        }
        // Fallback: try ALL states (handles checksum collisions)
        for (cs, base) in self.state_cache.iter().rev() {
            if *cs == last_state_checksum {
                continue; // already tried
            }
            if let Ok(result) = apply_diffs(base, diff_data) {
                let result_cs = compute_checksum(&result);
                if result_cs == expected_checksum {
                    return Some(result);
                }
            }
        }
        None
    }

    /// Process a GamePlayStatusMessage, returning the assembled state bytes.
    /// Returns `Ok(None)` if waiting for more chunks (non-tail message).
    /// Returns `Ok(Some(bytes))` when assembly is complete.
    pub fn process(
        &mut self,
        msg: &GamePlayStatusMessage,
    ) -> Result<Option<Vec<u8>>, DecodeError> {
        let is_diff = msg.flags & opcodes::FLAG_GAMESTATE_CONTAINS_DIFFS != 0;
        let is_head = msg.flags & opcodes::FLAG_GAMESTATE_HEAD != 0;
        let is_tail = msg.flags & opcodes::FLAG_GAMESTATE_TAIL != 0;

        if is_head {
            if !self.assembly_buffer.is_empty() {
                tracing::warn!(
                    "GamestateHead with non-empty assembly buffer, discarding {} bytes",
                    self.assembly_buffer.len()
                );
            }
            self.assembly_buffer.clear();
        }

        self.assembly_buffer.extend_from_slice(&msg.state_buf_raw);

        if !is_tail {
            return Ok(None);
        }

        let assembled = std::mem::take(&mut self.assembly_buffer);

        let final_state = if is_diff {
            if self.state_cache.is_empty() {
                return Err(DecodeError::UnexpectedEof {
                    context: "diff tail without prior state",
                });
            }

            match self.find_diff_base(&assembled, msg.last_state_checksum, msg.checksum) {
                Some(result) => {
                    // Validate size
                    if msg.undiffed_buffer_size != 0
                        && result.len() != msg.undiffed_buffer_size as usize
                    {
                        return Err(DecodeError::DiffSizeMismatch {
                            expected: msg.undiffed_buffer_size,
                            got: result.len() as u32,
                        });
                    }
                    result
                }
                None => {
                    return Err(DecodeError::InvalidChecksum {
                        expected: msg.checksum,
                        got: 0,
                    });
                }
            }
        } else {
            let checksum = compute_checksum(&assembled);
            if checksum != msg.checksum {
                return Err(DecodeError::InvalidChecksum {
                    expected: msg.checksum,
                    got: checksum,
                });
            }
            assembled
        };

        self.cache_state(&final_state);
        Ok(Some(final_state))
    }

    /// Reset between games.
    pub fn reset(&mut self) {
        self.assembly_buffer.clear();
        self.state_cache.clear();
    }
}

// ============================================================
// Element types
// ============================================================

/// A parsed state element from the StateBuf.
#[derive(Debug, Clone)]
pub enum StateElement {
    PlayerStatus(PlayerStatusElement),
    TurnStep(TurnStepElement),
    Thing(ThingElement),
    Other { element_type: u32, raw: Vec<u8> },
}

#[derive(Debug, Clone)]
pub struct PlayerStatusElement {
    pub life: Vec<i16>,
    pub hand_count: Vec<i16>,
    pub library_count: Vec<i16>,
    pub graveyard_count: Vec<i16>,
    pub time_left: Vec<i32>,
    pub active_player: u8,
}

#[derive(Debug, Clone)]
pub struct TurnStepElement {
    pub turn_number: i32,
    pub phase: u8,
}

#[derive(Debug, Clone)]
pub struct ThingElement {
    pub from_zone: i32,
    pub props: HashMap<u32, PropertyValue>,
}

#[derive(Debug, Clone)]
pub enum PropertyValue {
    Int(i32),
    Str(String),
    List(HashMap<u32, PropertyValue>),
}

// ============================================================
// Element parsing
// ============================================================

/// Parse state elements from an assembled state buffer.
pub fn parse_elements(data: &[u8]) -> Result<Vec<StateElement>, DecodeError> {
    let mut cursor = Cursor::new(data);
    let mut elements = Vec::new();
    let mut buf4 = [0u8; 4];

    while (cursor.position() as usize) < data.len() {
        let elem_start = cursor.position() as usize;

        // total_size: i32 (includes this 8-byte header)
        if cursor.read_exact(&mut buf4).is_err() {
            break;
        }
        let total_size = i32::from_le_bytes(buf4);
        if total_size < 8 {
            tracing::warn!("element total_size {} < 8, skipping rest", total_size);
            break;
        }

        // element_type: u32
        cursor
            .read_exact(&mut buf4)
            .map_err(|_| DecodeError::UnexpectedEof {
                context: "element type",
            })?;
        let element_type = u32::from_le_bytes(buf4);

        let payload_size = (total_size as usize) - 8;
        let payload_start = cursor.position() as usize;

        if payload_start + payload_size > data.len() {
            tracing::warn!("element payload extends past buffer, skipping");
            break;
        }

        let payload = &data[payload_start..payload_start + payload_size];

        let element = match element_type {
            opcodes::STATE_ELEM_THING => match parse_thing_element(payload) {
                Ok(Some(thing)) => StateElement::Thing(thing),
                Ok(None) => {
                    // THINGNUMBER absent — discard
                    cursor
                        .seek(SeekFrom::Start((elem_start + total_size as usize) as u64))
                        .ok();
                    continue;
                }
                Err(_) => {
                    tracing::warn!("failed to parse ThingElement, storing as Other");
                    StateElement::Other {
                        element_type,
                        raw: payload.to_vec(),
                    }
                }
            },
            opcodes::STATE_ELEM_TURN_STEP => match parse_turn_step_element(payload) {
                Ok(ts) => StateElement::TurnStep(ts),
                Err(_) => StateElement::Other {
                    element_type,
                    raw: payload.to_vec(),
                },
            },
            opcodes::STATE_ELEM_PLAYER_STATUS => match parse_player_status_element(payload) {
                Ok(ps) => StateElement::PlayerStatus(ps),
                Err(_) => StateElement::Other {
                    element_type,
                    raw: payload.to_vec(),
                },
            },
            opcodes::STATE_ELEM_MINI_CHANGE => {
                tracing::warn!("MiniChange element encountered (type 200), storing as Other");
                StateElement::Other {
                    element_type,
                    raw: payload.to_vec(),
                }
            }
            _ => StateElement::Other {
                element_type,
                raw: payload.to_vec(),
            },
        };

        elements.push(element);

        // Advance cursor to next element
        cursor
            .seek(SeekFrom::Start((elem_start + total_size as usize) as u64))
            .map_err(|_| DecodeError::UnexpectedEof {
                context: "element seek",
            })?;
    }

    Ok(elements)
}

fn parse_turn_step_element(payload: &[u8]) -> Result<TurnStepElement, DecodeError> {
    if payload.len() < 8 {
        return Err(DecodeError::UnexpectedEof {
            context: "TurnStep too short",
        });
    }
    let turn_number = i32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    // GamePhase is stored as i32 but we only need the low byte
    let phase = payload[4];
    Ok(TurnStepElement {
        turn_number,
        phase,
    })
}

fn parse_player_status_element(payload: &[u8]) -> Result<PlayerStatusElement, DecodeError> {
    let mut cursor = Cursor::new(payload);
    let mut buf4 = [0u8; 4];
    let mut buf2 = [0u8; 2];

    let life = read_i16_array(&mut cursor, &mut buf4, &mut buf2)?;
    let hand_count = read_i16_array(&mut cursor, &mut buf4, &mut buf2)?;
    let library_count = read_i16_array(&mut cursor, &mut buf4, &mut buf2)?;
    let graveyard_count = read_i16_array(&mut cursor, &mut buf4, &mut buf2)?;
    let time_left = read_i32_array(&mut cursor, &mut buf4)?;

    // background_image_names: string[] — read and discard
    if cursor.read_exact(&mut buf4).is_ok() {
        let str_count = i32::from_le_bytes(buf4);
        for _ in 0..str_count {
            // Each wide string: int32 char_count + char_count*2 bytes
            if cursor.read_exact(&mut buf4).is_err() {
                break;
            }
            let char_count = i32::from_le_bytes(buf4);
            if char_count > 0 {
                cursor
                    .seek(SeekFrom::Current(char_count as i64 * 2))
                    .ok();
            }
        }
    }

    // ActivePlayer: byte
    let mut buf1 = [0u8; 1];
    let active_player = if cursor.read_exact(&mut buf1).is_ok() {
        buf1[0]
    } else {
        0
    };

    Ok(PlayerStatusElement {
        life,
        hand_count,
        library_count,
        graveyard_count,
        time_left,
        active_player,
    })
}

fn read_i16_array(
    cursor: &mut Cursor<&[u8]>,
    buf4: &mut [u8; 4],
    buf2: &mut [u8; 2],
) -> Result<Vec<i16>, DecodeError> {
    cursor
        .read_exact(buf4)
        .map_err(|_| DecodeError::UnexpectedEof {
            context: "i16 array count",
        })?;
    let count = i32::from_le_bytes(*buf4);
    let mut values = Vec::with_capacity(count.max(0) as usize);
    for _ in 0..count {
        cursor
            .read_exact(buf2)
            .map_err(|_| DecodeError::UnexpectedEof {
                context: "i16 array value",
            })?;
        values.push(i16::from_le_bytes(*buf2));
    }
    Ok(values)
}

fn read_i32_array(
    cursor: &mut Cursor<&[u8]>,
    buf4: &mut [u8; 4],
) -> Result<Vec<i32>, DecodeError> {
    cursor
        .read_exact(buf4)
        .map_err(|_| DecodeError::UnexpectedEof {
            context: "i32 array count",
        })?;
    let count = i32::from_le_bytes(*buf4);
    let mut values = Vec::with_capacity(count.max(0) as usize);
    for _ in 0..count {
        cursor
            .read_exact(buf4)
            .map_err(|_| DecodeError::UnexpectedEof {
                context: "i32 array value",
            })?;
        values.push(i32::from_le_bytes(*buf4));
    }
    Ok(values)
}

fn parse_thing_element(payload: &[u8]) -> Result<Option<ThingElement>, DecodeError> {
    if payload.len() < 4 {
        return Err(DecodeError::UnexpectedEof {
            context: "ThingElement: from_zone",
        });
    }
    let from_zone = i32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let props = parse_property_container(&payload[4..])?;

    // Check for THINGNUMBER
    let thing_key = opcodes::THINGNUMBER & opcodes::PROP_KEY_MASK;
    if !props.contains_key(&thing_key) {
        tracing::warn!("ThingElement without THINGNUMBER, discarding");
        return Ok(None);
    }

    Ok(Some(ThingElement { from_zone, props }))
}

fn parse_property_container(data: &[u8]) -> Result<HashMap<u32, PropertyValue>, DecodeError> {
    let mut props = HashMap::new();
    let mut cursor = Cursor::new(data);
    let mut buf4 = [0u8; 4];

    loop {
        if cursor.read_exact(&mut buf4).is_err() {
            break;
        }
        let key_with_type = u32::from_le_bytes(buf4);
        if key_with_type == 0 {
            break; // terminator
        }

        let type_tag = key_with_type & opcodes::PROP_TYPE_MASK;
        let mut key = key_with_type & opcodes::PROP_KEY_MASK;

        match type_tag {
            opcodes::PROP_TYPE_INT8 => {
                let mut buf1 = [0u8; 1];
                cursor
                    .read_exact(&mut buf1)
                    .map_err(|_| DecodeError::UnexpectedEof {
                        context: "PropertyContainer: Int8 value",
                    })?;
                props.insert(key, PropertyValue::Int(buf1[0] as i8 as i32));
            }
            opcodes::PROP_TYPE_INT32 => {
                // Key remapping: (keyWithType & 0xD7FFFFFF) | 0x20000000, then mask to key
                let remapped = (key_with_type & 0xD7FFFFFF) | 0x20000000;
                key = remapped & opcodes::PROP_KEY_MASK;
                cursor
                    .read_exact(&mut buf4)
                    .map_err(|_| DecodeError::UnexpectedEof {
                        context: "PropertyContainer: Int32 value",
                    })?;
                props.insert(key, PropertyValue::Int(i32::from_le_bytes(buf4)));
            }
            opcodes::PROP_TYPE_STRING => {
                let mut buf2 = [0u8; 2];
                cursor
                    .read_exact(&mut buf2)
                    .map_err(|_| DecodeError::UnexpectedEof {
                        context: "PropertyContainer: String length",
                    })?;
                let length = u16::from_le_bytes(buf2);

                if length == 0xFFFF {
                    // String table reference
                    cursor
                        .read_exact(&mut buf4)
                        .map_err(|_| DecodeError::UnexpectedEof {
                            context: "PropertyContainer: StringTable index",
                        })?;
                    let idx = u32::from_le_bytes(buf4);
                    props.insert(key, PropertyValue::Str(format!("<strtable:{idx}>")));
                } else {
                    let mut str_bytes = vec![0u8; length as usize];
                    cursor
                        .read_exact(&mut str_bytes)
                        .map_err(|_| DecodeError::UnexpectedEof {
                            context: "PropertyContainer: String data",
                        })?;
                    // ISO-8859-1: each byte maps directly to a Unicode code point
                    let s: String = str_bytes.iter().map(|&b| b as char).collect();
                    props.insert(key, PropertyValue::Str(s));
                }
            }
            opcodes::PROP_TYPE_LIST => {
                // Nested attribute list — recursively parse until 0x00000000 terminator
                let remaining_start = cursor.position() as usize;
                let remaining = &data[remaining_start..];
                let nested = parse_property_container(remaining)?;
                // Advance cursor past the nested data + terminator
                let consumed = count_property_container_bytes(remaining);
                cursor.seek(SeekFrom::Current(consumed as i64)).ok();
                props.insert(key, PropertyValue::List(nested));
            }
            opcodes::PROP_TYPE_FUNCTION => {
                // No value, skip
            }
            opcodes::PROP_TYPE_STRING_CONSTANT => {
                // Abort the containing ThingElement
                tracing::warn!(
                    "StringConstant encountered in PropertyContainer, aborting element"
                );
                return Err(DecodeError::UnexpectedEof {
                    context: "StringConstant in PropertyContainer",
                });
            }
            _ => {
                tracing::warn!(
                    type_tag = type_tag,
                    "unknown PropertyContainer type tag, aborting element"
                );
                return Err(DecodeError::UnexpectedEof {
                    context: "unknown PropertyContainer type tag",
                });
            }
        }
    }

    Ok(props)
}

/// Count the number of bytes consumed by a property container (including the 0x00000000 terminator).
fn count_property_container_bytes(data: &[u8]) -> usize {
    let mut cursor = Cursor::new(data);
    let mut buf4 = [0u8; 4];

    loop {
        if cursor.read_exact(&mut buf4).is_err() {
            return cursor.position() as usize;
        }
        let key_with_type = u32::from_le_bytes(buf4);
        if key_with_type == 0 {
            return cursor.position() as usize;
        }

        let type_tag = key_with_type & opcodes::PROP_TYPE_MASK;
        match type_tag {
            opcodes::PROP_TYPE_INT8 => {
                cursor.seek(SeekFrom::Current(1)).ok();
            }
            opcodes::PROP_TYPE_INT32 => {
                cursor.seek(SeekFrom::Current(4)).ok();
            }
            opcodes::PROP_TYPE_STRING => {
                let mut buf2 = [0u8; 2];
                if cursor.read_exact(&mut buf2).is_err() {
                    return cursor.position() as usize;
                }
                let length = u16::from_le_bytes(buf2);
                if length == 0xFFFF {
                    cursor.seek(SeekFrom::Current(4)).ok();
                } else {
                    cursor.seek(SeekFrom::Current(length as i64)).ok();
                }
            }
            opcodes::PROP_TYPE_LIST => {
                let pos = cursor.position() as usize;
                let remaining = &data[pos..];
                let consumed = count_property_container_bytes(remaining);
                cursor.seek(SeekFrom::Current(consumed as i64)).ok();
            }
            opcodes::PROP_TYPE_FUNCTION => { /* no value */ }
            _ => {
                return cursor.position() as usize;
            }
        }
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- Checksum tests ---

    #[test]
    fn test_checksum_empty() {
        assert_eq!(compute_checksum(&[]), opcodes::CHECKSUM_SEED);
    }

    #[test]
    fn test_checksum_deterministic() {
        let data = vec![0x01, 0x02, 0x03];
        let c1 = compute_checksum(&data);
        let c2 = compute_checksum(&data);
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_checksum_differs_for_different_data() {
        let c1 = compute_checksum(&[0x01]);
        let c2 = compute_checksum(&[0x02]);
        assert_ne!(c1, c2);
    }

    // --- Diff algorithm tests ---

    #[test]
    fn test_apply_diffs_literal() {
        // Short literal: byte 0x03 means "3 literal bytes follow"
        let old_state = vec![0xAA, 0xBB, 0xCC];
        let diff = vec![0x03, 0x11, 0x22, 0x33];
        let result = apply_diffs(&old_state, &diff).unwrap();
        assert_eq!(result, vec![0x11, 0x22, 0x33]);
    }

    #[test]
    fn test_apply_diffs_copy() {
        // 0x00 copy: uint16 count=3, int16 seek=0 (from start)
        let old_state = vec![0xAA, 0xBB, 0xCC, 0xDD];
        let diff = vec![
            0x00, 0x03, 0x00, // count = 3
            0x00, 0x00, // seek = 0 (absolute from current cursor=0)
        ];
        let result = apply_diffs(&old_state, &diff).unwrap();
        assert_eq!(result, vec![0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn test_apply_diffs_short_copy() {
        // 0x83 = 0x80 | 3 → count=3, then sbyte seek
        let old_state = vec![0xAA, 0xBB, 0xCC, 0xDD];
        let diff = vec![0x83, 0x00]; // count=3, seek=0
        let result = apply_diffs(&old_state, &diff).unwrap();
        assert_eq!(result, vec![0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn test_apply_diffs_out_of_bounds() {
        let old_state = vec![0xAA];
        let diff = vec![0x00, 0x05, 0x00, 0x00, 0x00]; // copy 5, seek 0 — out of bounds
        let result = apply_diffs(&old_state, &diff);
        assert!(result.is_err());
    }

    // --- StateBufProcessor tests ---

    #[test]
    fn test_processor_full_state() {
        let mut proc = StateBufProcessor::new();
        let state_data = vec![0x01, 0x02, 0x03];
        let checksum = compute_checksum(&state_data);

        let msg = GamePlayStatusMessage {
            time_left: [0; 8],
            player_waiting_for: 0,
            game_id: 1,
            state_size: state_data.len() as u32,
            undiffed_buffer_size: 0,
            n_state_elems: 0,
            priority_player: 0,
            checksum,
            last_state_checksum: 0,
            flags: opcodes::FLAG_GAMESTATE_HEAD | opcodes::FLAG_GAMESTATE_TAIL, // 0x06
            game_state_timestamp: 0,
            state_buf_raw: state_data.clone(),
        };

        let result = proc.process(&msg).unwrap();
        assert_eq!(result, Some(state_data));
    }

    #[test]
    fn test_processor_bad_checksum() {
        let mut proc = StateBufProcessor::new();
        let msg = GamePlayStatusMessage {
            time_left: [0; 8],
            player_waiting_for: 0,
            game_id: 1,
            state_size: 3,
            undiffed_buffer_size: 0,
            n_state_elems: 0,
            priority_player: 0,
            checksum: 12345, // wrong
            last_state_checksum: 0,
            flags: opcodes::FLAG_GAMESTATE_HEAD | opcodes::FLAG_GAMESTATE_TAIL,
            game_state_timestamp: 0,
            state_buf_raw: vec![0x01, 0x02, 0x03],
        };

        let result = proc.process(&msg);
        assert!(matches!(result, Err(DecodeError::InvalidChecksum { .. })));
    }

    // --- Element parsing tests ---

    /// Helper: wrap payload bytes in a state element header (4-byte size + 4-byte type).
    fn make_element_bytes(elem_type: u32, payload: &[u8]) -> Vec<u8> {
        let total_size = (8 + payload.len()) as i32;
        let mut data = Vec::new();
        data.extend_from_slice(&total_size.to_le_bytes());
        data.extend_from_slice(&elem_type.to_le_bytes());
        data.extend_from_slice(payload);
        data
    }

    #[test]
    fn test_parse_thing_element_simple() {
        let mut payload = Vec::new();
        // from_zone: i32 = 2 (Library)
        payload.extend_from_slice(&2i32.to_le_bytes());
        // PropertyContainer: one Int8 property (THINGNUMBER), then terminator
        let key = opcodes::PROP_TYPE_INT8 | (opcodes::THINGNUMBER & opcodes::PROP_KEY_MASK);
        payload.extend_from_slice(&key.to_le_bytes());
        payload.push(42); // int8 value
        // terminator
        payload.extend_from_slice(&0u32.to_le_bytes());

        let elem_data = make_element_bytes(opcodes::STATE_ELEM_THING, &payload);
        let elements = parse_elements(&elem_data).unwrap();
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            StateElement::Thing(thing) => {
                assert_eq!(thing.from_zone, 2);
                let key = opcodes::THINGNUMBER & opcodes::PROP_KEY_MASK;
                assert!(thing.props.contains_key(&key));
            }
            _ => panic!("expected Thing"),
        }
    }

    #[test]
    fn test_parse_turn_step_element() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&5i32.to_le_bytes()); // TurnNumber
        payload.extend_from_slice(&4u32.to_le_bytes()); // GamePhase = PreCombatMain

        let elem_data = make_element_bytes(opcodes::STATE_ELEM_TURN_STEP, &payload);
        let elements = parse_elements(&elem_data).unwrap();
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            StateElement::TurnStep(ts) => {
                assert_eq!(ts.turn_number, 5);
                assert_eq!(ts.phase, 4);
            }
            _ => panic!("expected TurnStep"),
        }
    }

    #[test]
    fn test_parse_unknown_element_type() {
        let payload = vec![0x01, 0x02, 0x03];
        let elem_data = make_element_bytes(255, &payload);
        let elements = parse_elements(&elem_data).unwrap();
        assert_eq!(elements.len(), 1);
        assert!(matches!(
            elements[0],
            StateElement::Other {
                element_type: 255,
                ..
            }
        ));
    }

    // --- Golden file tests ---

    #[test]
    fn test_golden_file_statebuf_assembly() {
        let data = std::fs::read("tests/fixtures/golden_v1.bin").unwrap();
        let messages = crate::protocol::framing::parse_messages(&data).unwrap();

        let mut processor = StateBufProcessor::new();
        let mut success_count = 0;
        let mut pending_count = 0;
        let mut errors = Vec::new();

        for msg in &messages {
            if msg.opcode != 1153 && msg.opcode != 1156 {
                continue;
            }
            let fls = match crate::protocol::fls::decode_fls(msg.clone()) {
                Ok(f) => f,
                Err(_) => continue,
            };
            let meta = match fls {
                crate::protocol::fls::FlsMessage::GsMessage { meta, .. } => meta,
                _ => continue,
            };
            let game_msg = match crate::protocol::game_messages::decode_game_message(&meta) {
                Ok(g) => g,
                Err(_) => continue,
            };
            let play_status = match game_msg {
                crate::protocol::game_messages::GameMessage::GamePlayStatus(ps) => ps,
                _ => continue,
            };

            match processor.process(&play_status) {
                Ok(Some(_)) => success_count += 1,
                Ok(None) => pending_count += 1,
                Err(e) => errors.push(format!("{e}")),
            }
        }

        eprintln!(
            "StateBuf assembly: {success_count} success, {pending_count} pending, {} errors",
            errors.len()
        );
        for (i, e) in errors.iter().enumerate().take(5) {
            eprintln!("  error {i}: {e}");
        }
        assert!(errors.is_empty(), "expected no assembly errors");
        assert!(success_count > 0, "expected successful assemblies");
    }

    #[test]
    fn test_golden_file_element_parsing() {
        let data = std::fs::read("tests/fixtures/golden_v1.bin").unwrap();
        let messages = crate::protocol::framing::parse_messages(&data).unwrap();

        let mut processor = StateBufProcessor::new();
        let mut total_elements = 0;
        let mut count_mismatches = 0;
        let mut parse_errors = 0;

        for msg in &messages {
            if msg.opcode != 1153 && msg.opcode != 1156 {
                continue;
            }
            let fls = match crate::protocol::fls::decode_fls(msg.clone()) {
                Ok(f) => f,
                Err(_) => continue,
            };
            let meta = match fls {
                crate::protocol::fls::FlsMessage::GsMessage { meta, .. } => meta,
                _ => continue,
            };
            let game_msg = match crate::protocol::game_messages::decode_game_message(&meta) {
                Ok(g) => g,
                Err(_) => continue,
            };
            let ps = match game_msg {
                crate::protocol::game_messages::GameMessage::GamePlayStatus(ps) => ps,
                _ => continue,
            };

            let expected_elems = ps.n_state_elems;
            let assembled = match processor.process(&ps) {
                Ok(Some(a)) => a,
                Ok(None) => continue,
                Err(_) => continue,
            };

            match parse_elements(&assembled) {
                Ok(elems) => {
                    total_elements += elems.len();
                    if elems.len() != expected_elems as usize {
                        count_mismatches += 1;
                    }
                }
                Err(e) => {
                    eprintln!("element parse error: {e}");
                    parse_errors += 1;
                }
            }
        }

        eprintln!(
            "Element parsing: {total_elements} total, {count_mismatches} count mismatches, {parse_errors} errors"
        );
        assert_eq!(parse_errors, 0, "no element parse errors");
    }
}
