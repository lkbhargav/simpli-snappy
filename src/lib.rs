use serde_json::Value;
use snap::raw::max_compress_len;
use snap::raw::{decompress_len, Decoder, Encoder};
use std::error::Error;
use std::str;

pub struct Snappier {
  custom_json_compression_logic: bool,
  decoder: Decoder,
  encoder: Encoder,
}

impl Snappier {
  /// custom => Custom compression if required, should be set as true
  pub fn new(custom_json_compression_logic: bool) -> Self {
    let decoder = Decoder::new();
    let encoder = Encoder::new();

    Snappier {
      custom_json_compression_logic,
      decoder,
      encoder,
    }
  }

  pub fn decode(& mut self, byte_arr: Vec<u8>) -> Result<String, Box<dyn Error>> {
    // converting Vec<u8> to &[u8]
    let byte_arr_inp: &[u8] = &byte_arr;

    let decompress_bytes_len = decompress_len(byte_arr_inp)?;

    let mut byte_arr_out: Vec<u8> = vec![0; decompress_bytes_len];

    self.decoder.decompress(byte_arr_inp, &mut byte_arr_out)?;

    let tmp = str::from_utf8(&byte_arr_out)?;

    let mut resp: String = tmp.to_string();

    if self.custom_json_compression_logic {
      resp = Self::custom_decode(tmp);
      if resp.is_empty() {
        resp = tmp.to_string();
      }
    }

    Ok(resp)
  }

  pub fn encode(
    & mut self,
    input_to_compress: &str,
    keys: Vec<&str>,
  ) -> Result<Vec<u8>, Box<dyn Error>> {
    // if input is empty we just return
    if input_to_compress.is_empty() {
      return Ok(vec![]);
    }

    let mut res: String = input_to_compress.to_string();

    if self.custom_json_compression_logic {
      res = Self::custom_encode(input_to_compress, keys);
      if res.is_empty() {
        res = input_to_compress.to_string();
      }
    }

    let inp_as_bytes = res.as_bytes();

    let compress_bytes_len = max_compress_len(inp_as_bytes.len());

    let mut output = vec![0; compress_bytes_len];

    self.encoder.compress(&inp_as_bytes, &mut output)?;

    let res = Snappier::get_index_of_zero_starting_index(&output);

    if res.1 {
      output = (&output[..res.0]).to_vec();
    }

    Ok(output)
  }

  fn custom_decode(inp: &str) -> String {
    if !inp.starts_with("[") {
      return String::new();
    }
    let (keys_str, value) = inp.split_once("]").unwrap();
    let keys_str = keys_str.replace("[", "");
    let keys: Vec<&str> = keys_str.split(",").collect();
    let mut value = value.replace("^~", "`{`");
    let mut n = keys.len();
    while n > 0 {
      value = value.replace(
        format!("`{}", n - 1).as_str(),
        format!("\"{}", keys[n - 1]).as_str(),
      );
      n -= 1;
    }
    value = value.replace("`{", "},{");
    value = value.replace("`f", "false");
    value = value.replace("`t", "true");
    value = value.replace("`!", "\":\"");
    value
  }
  fn custom_encode(inp: &str, keys: Vec<&str>) -> String {
    // check to see if it's a valid JSON string
    let val: Value = serde_json::from_str(inp).unwrap_or_default();
    if val == Value::Null {
      return String::new();
    }
    // dropping the value immeditely as it won't be used anymore
    drop(val);
    let mut compressed: String;
    compressed = inp.replace("\n", "");
    compressed = compressed.replace("\r", "");
    compressed = compressed.replace("\":\"", "`!");
    compressed = compressed.replace("true", "`t");
    compressed = compressed.replace("false", "`f");
    compressed = compressed.replace("},{", "`{");
    let mut counter: u16 = 0;
    let mut key_array = vec![];
    for key in keys {
      compressed = compressed.replace(
        format!("\"{}", key).as_str(),
        format!("`{}", counter).as_str(),
      );
      key_array.push(key);
      counter += 1;
    }
    compressed = format!("[{}]{}", key_array.join(","), compressed);
    compressed
  }

  fn get_index_of_zero_starting_index(inp: &Vec<u8>) -> (usize, bool) {
    if inp[inp.len()-1] > 0 {
      return (0, false);
    }

    let mut min = 0;
    let mut max = inp.len();
    let mut current_search_index: usize = (max + min) / 2;

    let mut key_index_found = 0;

    loop {
      if inp[current_search_index] == 0
      // searching to the left to see if it is zero or not
        && ((current_search_index > 0 && inp[current_search_index - 1] > 0)
          || current_search_index == 0)
      {
        let mut tmp_index = current_search_index;
        let mut false_positive: bool = false;

        // searching for the next 5 bytes also to be zero
        if tmp_index < inp.len()-1 && inp[tmp_index+1] == 0 {
          while tmp_index+1 < inp.len()-1 {
            if inp[tmp_index+2] > 0 {
              false_positive = true;
              break;
            }
            tmp_index+=1;
          }
        }

        if tmp_index < inp.len()-1 && inp[tmp_index+1] > 0 {
          min = tmp_index+1;
          false_positive = true;
        }

        if !false_positive {
          key_index_found = current_search_index;
          break;
        }
      }

      if inp[current_search_index] > 0 {
        min = current_search_index;
      }

      // zero can come in at 2 conditions => 1. In between the valid bytes; 2. Once the valid bytes end | logic here will make sure to avoid false positivies when we are dealing with first condition
      if inp[current_search_index] == 0 {
        let mut tmp_index = current_search_index;
        let mut counter = 0;

        while tmp_index+1 < inp.len()-1 {
          counter+=1;

          if inp[tmp_index+1] > 0 {
            min = tmp_index+1;
            break;
          }
          tmp_index+=1;

          if counter == 10 {
            max = current_search_index;
            break;
          }
        }
      }

      current_search_index = (max+min)/2;
    }

    (key_index_found, true)
  }
}



#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn basic_encoding() -> Result<(), Box<dyn Error>> {
    let mut snappier = Snappier::new(false);

    assert_eq!(
      snappier.encode("just a test", vec![])?,
      vec![11, 40, 106, 117, 115, 116, 32, 97, 32, 116, 101, 115, 116]
    );

    Ok(())
  }

  #[test]
  fn basic_decoding() -> Result<(), Box<dyn Error>> {
    let mut snappier = Snappier::new(false);

    assert_eq!(
      snappier.decode(vec![
        11, 40, 106, 117, 115, 116, 32, 97, 32, 116, 101, 115, 116
      ])?,
      "just a test"
    );

    Ok(())
  }

  #[test]
  fn basic_encoding_json() -> Result<(), Box<dyn Error>> {
    let mut snappier = Snappier::new(false);

    assert_eq!(
      snappier.encode("{\"name\": \"Sky\", \"age\": 5}", vec![])?,
      vec![
        25, 96, 123, 34, 110, 97, 109, 101, 34, 58, 32, 34, 83, 107, 121, 34, 44, 32, 34, 97, 103,
        101, 34, 58, 32, 53, 125
      ]
    );

    Ok(())
  }

  #[test]
  fn basic_decoding_json() -> Result<(), Box<dyn Error>> {
    let mut snappier = Snappier::new(false);

    assert_eq!(
      snappier.decode(vec![
        25, 96, 123, 34, 110, 97, 109, 101, 34, 58, 32, 34, 83, 107, 121, 34, 44, 32, 34, 97, 103,
        101, 34, 58, 32, 53, 125
      ])?,
      "{\"name\": \"Sky\", \"age\": 5}"
    );

    Ok(())
  }

  #[test]
  fn basic_encoding_custom_json() -> Result<(), Box<dyn Error>> {
    let mut snappier = Snappier::new(true);

    assert_eq!(
      snappier.encode("{\"name\": \"Sky\", \"age\": 5}", vec![])?,
      vec![
        27, 104, 91, 93, 123, 34, 110, 97, 109, 101, 34, 58, 32, 34, 83, 107, 121, 34, 44, 32, 34,
        97, 103, 101, 34, 58, 32, 53, 125
      ]
    );

    Ok(())
  }

  #[test]
  fn basic_decoding_custom_json() -> Result<(), Box<dyn Error>> {
    let mut snappier = Snappier::new(true);

    assert_eq!(
      snappier.decode(vec![
        27, 104, 91, 93, 123, 34, 110, 97, 109, 101, 34, 58, 32, 34, 83, 107, 121, 34, 44, 32, 34,
        97, 103, 101, 34, 58, 32, 53, 125
      ])?,
      "{\"name\": \"Sky\", \"age\": 5}"
    );

    Ok(())
  }

  #[test]
  fn basic_encoding_custom_json_with_keys() -> Result<(), Box<dyn Error>> {
    let mut snappier = Snappier::new(true);

    assert_eq!(
      snappier.encode("{\"name\": \"Sky\", \"age\": 5}", vec!["name", "age"])?,
      vec![
        30, 116, 91, 110, 97, 109, 101, 44, 97, 103, 101, 93, 123, 96, 48, 34, 58, 32, 34, 83, 107,
        121, 34, 44, 32, 96, 49, 34, 58, 32, 53, 125
      ]
    );

    Ok(())
  }

  #[test]
  fn basic_decoding_custom_json_with_keys() -> Result<(), Box<dyn Error>> {
    let mut snappier = Snappier::new(true);

    assert_eq!(
      snappier.decode(vec![
        30, 116, 91, 110, 97, 109, 101, 44, 97, 103, 101, 93, 123, 96, 48, 34, 58, 32, 34, 83, 107,
        121, 34, 44, 32, 96, 49, 34, 58, 32, 53, 125
      ])?,
      "{\"name\": \"Sky\", \"age\": 5}"
    );

    Ok(())
  }

  #[test]
  fn basic_encoding_custom_json_with_keys_plus_special_chars() -> Result<(), Box<dyn Error>> {
    let mut snappier = Snappier::new(true);

    assert_eq!(
      snappier.encode("{\r\"name\": \r\"Sky\", \r\"age\": 5}", vec!["name", "age"])?,
      vec![
        30, 116, 91, 110, 97, 109, 101, 44, 97, 103, 101, 93, 123, 96, 48, 34, 58, 32, 34, 83, 107,
        121, 34, 44, 32, 96, 49, 34, 58, 32, 53, 125
      ]
    );

    Ok(())
  }

  #[test]
  fn basic_encoding_plus_special_chars() -> Result<(), Box<dyn Error>> {
    let mut snappier = Snappier::new(false);

    assert_eq!(
      snappier.encode("{\r\"name\": \r\"Sky\", \r\"age\": 5}", vec![])?,
      vec![
        28, 108, 123, 13, 34, 110, 97, 109, 101, 34, 58, 32, 13, 34, 83, 107, 121, 34, 44, 32, 13,
        34, 97, 103, 101, 34, 58, 32, 53, 125
      ]
    );

    Ok(())
  }

  #[test]
  fn basic_decoding_plus_special_chars() -> Result<(), Box<dyn Error>> {
    let mut snappier = Snappier::new(false);

    assert_eq!(
      snappier.decode(vec![
        28, 108, 123, 13, 34, 110, 97, 109, 101, 34, 58, 32, 13, 34, 83, 107, 121, 34, 44, 32, 13,
        34, 97, 103, 101, 34, 58, 32, 53, 125
      ])?,
      "{\r\"name\": \r\"Sky\", \r\"age\": 5}"
    );

    Ok(())
  }

  #[test]
  fn basic_encoding_on_large_string() -> Result<(), Box<dyn Error>> {
    let mut snappier = Snappier::new(true);

    assert_eq!(
      snappier.encode("{\"text\": \"One of the most exciting things is the ability to test the latest and greatest web platform features like ES6 modules, service workers, and streams. With headless chrome, you can write apps and test those apps with up-to-date rendering. The other thing that it unlocks is these awesome functionalities like network throttling, device emulation, and code coverage.\"}", vec!["text"])?,
      vec![
        250, 2, 184, 91, 116, 101, 120, 116, 93, 123, 96, 48, 34, 58, 32, 34, 79, 110, 101, 32, 111, 102, 32, 116, 
        104, 101, 32, 109, 111, 115, 116, 32, 101, 120, 99, 105, 116, 105, 110, 103, 32, 116, 104, 105, 110, 103, 
        115, 32, 105, 115, 5, 28, 56, 97, 98, 105, 108, 105, 116, 121, 32, 116, 111, 32, 116, 101, 115, 116, 5, 20, 
        4, 108, 97, 5, 11, 28, 97, 110, 100, 32, 103, 114, 101, 97, 5, 13, 224, 119, 101, 98, 32, 112, 108, 97, 116, 
        102, 111, 114, 109, 32, 102, 101, 97, 116, 117, 114, 101, 115, 32, 108, 105, 107, 101, 32, 69, 83, 54, 32, 
        109, 111, 100, 117, 108, 101, 115, 44, 32, 115, 101, 114, 118, 105, 99, 101, 32, 119, 111, 114, 107, 101, 114, 
        115, 44, 32, 1, 70, 192, 115, 116, 114, 101, 97, 109, 115, 46, 32, 87, 105, 116, 104, 32, 104, 101, 97, 100, 
        108, 101, 115, 115, 32, 99, 104, 114, 111, 109, 101, 44, 32, 121, 111, 117, 32, 99, 97, 110, 32, 119, 114, 
        105, 116, 101, 32, 97, 112, 112, 115, 5, 124, 5, 120, 12, 116, 104, 111, 115, 13, 20, 0, 119, 1, 61, 120, 117, 
        112, 45, 116, 111, 45, 100, 97, 116, 101, 32, 114, 101, 110, 100, 101, 114, 105, 110, 103, 46, 32, 84, 104, 
        101, 32, 111, 116, 104, 101, 114, 9, 222, 56, 32, 116, 104, 97, 116, 32, 105, 116, 32, 117, 110, 108, 111, 99, 
        107, 17, 237, 1, 73, 84, 119, 101, 115, 111, 109, 101, 32, 102, 117, 110, 99, 116, 105, 111, 110, 97, 108, 105, 
        116, 105, 101, 115, 9, 201, 84, 110, 101, 116, 119, 111, 114, 107, 32, 116, 104, 114, 111, 116, 116, 108, 105, 
        110, 103, 44, 32, 100, 101, 5, 207, 16, 101, 109, 117, 108, 97, 1, 49, 0, 44, 5, 155, 60, 99, 111, 100, 101, 32, 
        99, 111, 118, 101, 114, 97, 103, 101, 46, 34, 125
      ]
    );

    Ok(())
  }

  #[test]
  fn basic_encoding_on_large_string_2() -> Result<(), Box<dyn Error>> {
    let mut snappier = Snappier::new(true);

    assert_eq!(
      snappier.encode("[{\"name\": \"Bhargav\"}, {\"name\": \"Bhargav\"}, {\"name\": \"Bhargav\"}, {\"name\": \"Bhargav\"}, {\"name\": \"Bhargav\"}, {\"name\": \"Bhargav\"}]", vec!["name", "Bhargav"])?,
      vec![
        86, 104, 91, 110, 97, 109, 101, 44, 66, 104, 97, 114, 103, 97, 118, 93, 91, 123, 96, 48, 34, 58, 32, 96, 49, 34, 125, 44, 32, 230, 12, 0, 0, 93
      ]
    );

    Ok(())
  }

  #[test]
  fn test_get_index_of_zero_starting_index_basic() {
    let (val, found) = Snappier::get_index_of_zero_starting_index(&vec![1,2,3,4,5,6,0,0,0,0,0,0,0]);
    assert_eq!(val, 6);
    assert!(found);
  }

  #[test]
  fn test_get_index_of_zero_starting_index_1() {
    let val = Snappier::get_index_of_zero_starting_index(&vec![1,2,3,4,5,6,0,0,0]);
    assert_eq!(val.0, 6);
    assert!(val.1);
  }

  #[test]
  fn test_get_index_of_zero_starting_index_2() {
    let val = Snappier::get_index_of_zero_starting_index(&vec![1,2,3,4,5,6,0]);
    assert_eq!(val.0, 6);
    assert!(val.1);
  }

  #[test]
  fn test_get_index_of_zero_starting_index_3() {
    let val = Snappier::get_index_of_zero_starting_index(&vec![1,2,3,4,5,6]);
    assert_eq!(val.0, 0);
    assert!(!val.1);
  }

  #[test]
  fn test_get_index_of_zero_starting_index_4() {
    let val = Snappier::get_index_of_zero_starting_index(&vec![1,2,3,0,5,6,7]);
    assert_eq!(val.0, 0);
    assert!(!val.1);
  }

  #[test]
  fn test_get_index_of_zero_starting_index_5() {
    let val = Snappier::get_index_of_zero_starting_index(&vec![1,2,3,0,5,6,7,0]);
    assert_eq!(val.0, 7);
    assert!(val.1);
  }

  #[test]
  fn test_get_index_of_zero_starting_index_6() {
    let val = Snappier::get_index_of_zero_starting_index(&vec![1,2,3,0,0,5,6,7,0]);
    assert_eq!(val.0, 8);
    assert!(val.1);
  }

  #[test]
  fn test_get_index_of_zero_starting_index_7() {
    let val = Snappier::get_index_of_zero_starting_index(&vec![1,2,3,0,0,0,0,0,0,0,5,6,7,0]);
    assert_eq!(val.0, 13);
    assert!(val.1);
  }

  #[test]
  fn test_get_index_of_zero_starting_index_8() {
    let val = Snappier::get_index_of_zero_starting_index(&vec![1,2,3,0,0,0,0,0,0,0,0,0,0,0,0,5,6,7,0]);
    assert_eq!(val.0, 18);
    assert!(val.1);
  }

  #[test]
  fn test_get_index_of_zero_starting_index_9() {
    let val = Snappier::get_index_of_zero_starting_index(&vec![139, 11, 216, 103, 82, 80, 67, 32, 97, 110, 100, 32, 80, 114, 111, 116, 111, 98, 117, 102, 32, 112, 114, 111, 118, 105, 100, 101, 32, 97, 110, 32, 101, 97, 115, 121, 32, 119, 97, 121, 32, 116, 111, 32, 112, 114, 101, 99, 105, 115, 101, 108, 121, 32, 10, 32, 32, 32, 194, 2, 0, 56, 100, 101, 102, 105, 110, 101, 32, 97, 32, 115, 101, 114, 118, 105, 99, 1, 95, 124, 100, 32, 97, 117, 116, 111, 32, 103, 101, 110, 101, 114, 97, 116, 101, 32, 114, 101, 108, 105, 97, 98, 108, 101, 32, 99, 108, 105, 101, 110, 116, 32, 210, 104, 0, 104, 108, 105, 98, 114, 97, 114, 105, 101, 115, 32, 102, 111, 114, 32, 105, 79, 83, 44, 32, 65, 110, 100, 114, 111, 105, 100, 32, 1, 230, 8, 116, 104, 101, 5, 130, 8, 101, 114, 115, 13, 233, 8, 105, 110, 103, 214, 106, 0, 1, 75, 44, 98, 97, 99, 107, 32, 101, 110, 100, 46, 32, 84, 104, 17, 184, 88, 115, 32, 99, 97, 110, 32, 116, 97, 107, 101, 32, 97, 100, 118, 97, 110, 116, 97, 103, 101, 32, 111, 102, 9, 13, 8, 99, 101, 100, 214, 110, 0, 20, 115, 116, 114, 101, 97, 109, 1, 173, 1, 199, 184, 99, 111, 110, 110, 101, 99, 116, 105, 111, 110, 32, 102, 101, 97, 116, 117, 114, 101, 115, 32, 119, 104, 105, 99, 104, 32, 104, 101, 108, 112, 32, 115, 97, 118, 101, 32, 98, 97, 110, 100, 119, 105, 100, 116, 104, 44, 32, 210, 74, 1, 84, 100, 111, 32, 109, 111, 114, 101, 32, 111, 118, 101, 114, 32, 102, 101, 119, 101, 114, 32, 84, 67, 80, 29, 123, 0, 115, 5, 139, 5, 108, 20, 67, 80, 85, 32, 117, 115, 1, 233, 1, 158, 48, 98, 97, 116, 116, 101, 114, 121, 32, 108, 105, 102, 101, 46, 210, 124, 0, 69, 150, 12, 108, 97, 114, 103, 65, 112, 84, 102, 111, 108, 108, 111, 119, 115, 32, 72, 84, 84, 80, 32, 115, 101, 109, 97, 110, 116, 105, 99, 115, 9, 152, 1, 20, 72, 47, 50, 32, 98, 117, 116, 32, 119, 101, 32, 101, 120, 112, 108, 105, 99, 105, 116, 108, 218, 165, 2, 0, 97, 1, 105, 69, 57, 40, 102, 117, 108, 108, 45, 100, 117, 112, 108, 101, 120, 57, 123, 120, 46, 32, 87, 101, 32, 100, 105, 118, 101, 114, 103, 101, 32, 102, 114, 111, 109, 32, 116, 121, 112, 105, 99, 97, 108, 32, 82, 69, 83, 84, 32, 210, 234, 0, 20, 99, 111, 110, 118, 101, 110, 33, 202, 12, 115, 32, 97, 115, 1, 197, 56, 117, 115, 101, 32, 115, 116, 97, 116, 105, 99, 32, 112, 97, 116, 104, 73, 201, 92, 112, 101, 114, 102, 111, 114, 109, 97, 110, 99, 101, 32, 114, 101, 97, 115, 111, 110, 115, 32, 100, 117, 114, 105, 222, 189, 2, 48, 99, 97, 108, 108, 32, 100, 105, 115, 112, 97, 116, 99, 104, 1, 121, 12, 112, 97, 114, 115, 1, 78, 5, 25, 24, 112, 97, 114, 97, 109, 101, 116, 97, 37, 5, 231, 5, 136, 12, 44, 32, 113, 117, 33, 214, 29, 29, 210, 242, 0, 65, 34, 96, 112, 97, 121, 108, 111, 97, 100, 32, 98, 111, 100, 121, 32, 97, 100, 100, 115, 32, 108, 97, 116, 101, 110, 99, 121, 5, 30, 8, 99, 111, 109, 33, 121, 8, 105, 116, 121, 37, 114, 0, 104, 65, 206, 12, 97, 108, 115, 111, 33, 154, 16, 109, 97, 108, 105, 122, 222, 69, 3, 129, 126, 56, 116, 32, 111, 102, 32, 101, 114, 114, 111, 114, 115, 32, 116, 104, 97, 69, 56, 40, 98, 101, 108, 105, 101, 118, 101, 32, 97, 114, 101, 73, 244, 16, 100, 105, 114, 101, 99, 65, 71, 20, 97, 112, 112, 108, 105, 99, 133, 150, 20, 116, 111, 32, 65, 80, 73, 214, 244, 0, 33, 212, 12, 99, 97, 115, 101, 5, 115, 0, 110, 133, 59, 65, 186, 52, 32, 115, 116, 97, 116, 117, 115, 32, 99, 111, 100, 101, 115, 46, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(val.0, 715);
    assert!(val.1);
  }

  #[test]
  fn test_get_index_of_zero_starting_index_10() {
    let val = Snappier::get_index_of_zero_starting_index(&vec![126, 84, 91, 123, 34, 110, 97, 109, 101, 34, 58, 32, 34, 66, 104, 97, 114, 103, 97, 118, 34, 125, 44, 32, 254, 21, 0, 154, 21, 0, 0, 93, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(val.0, 32);
    assert!(val.1);
  }
}