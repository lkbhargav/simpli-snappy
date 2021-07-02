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

    let to_retain = self.encoder.compress(&inp_as_bytes, &mut output)?;

    output = (&output[..to_retain]).to_vec();

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
  fn test_complex_encoding() -> Result<(), Box<dyn Error>> {
      let inp = "The Rust Reference defines a raw string literal as starting with the character U+0072 (r), followed by zero or more of the character U+0023 (#) and a U+0022 (double-quote) character. The raw string body can contain any sequence of Unicode characters and is terminated only by another U+0022 (double-quote) character, followed by the same number of U+0023 (#) characters that preceded the opening U+0022 (double-quote) character.";

      let to_decode = vec![172, 3, 240, 121, 84, 104, 101, 32, 82, 117, 115, 116, 32, 82, 101, 102, 101, 114, 101, 110, 99, 101, 32, 100, 101, 102, 105, 110, 101, 115, 32, 97, 32, 114, 97, 119, 32, 115, 116, 114, 105, 110, 103, 32, 108, 105, 116, 101, 114, 97, 108, 32, 97, 115, 32, 115, 116, 97, 114, 116, 105, 110, 103, 32, 119, 105, 116, 104, 32, 116, 104, 101, 32, 99, 104, 97, 114, 97, 99, 116, 101, 114, 32, 85, 43, 48, 48, 55, 50, 32, 40, 114, 41, 44, 32, 102, 111, 108, 108, 111, 119, 101, 100, 32, 98, 121, 32, 122, 101, 114, 111, 32, 111, 114, 32, 109, 111, 114, 101, 32, 111, 102, 32, 116, 104, 101, 58, 54, 0, 144, 50, 51, 32, 40, 35, 41, 32, 97, 110, 100, 32, 97, 32, 85, 43, 48, 48, 50, 50, 32, 40, 100, 111, 117, 98, 108, 101, 45, 113, 117, 111, 116, 101, 41, 32, 99, 104, 13, 103, 4, 46, 32, 1, 183, 29, 158, 96, 98, 111, 100, 121, 32, 99, 97, 110, 32, 99, 111, 110, 116, 97, 105, 110, 32, 97, 110, 121, 32, 115, 101, 113, 117, 5, 209, 36, 111, 102, 32, 85, 110, 105, 99, 111, 100, 101, 25, 116, 0, 115, 5, 106, 112, 105, 115, 32, 116, 101, 114, 109, 105, 110, 97, 116, 101, 100, 32, 111, 110, 108, 121, 32, 98, 121, 32, 97, 110, 111, 116, 104, 101, 114, 126, 134, 0, 54, 226, 0, 33, 8, 40, 115, 97, 109, 101, 32, 110, 117, 109, 98, 101, 114, 5, 117, 4, 43, 48, 17, 215, 21, 187, 56, 115, 32, 116, 104, 97, 116, 32, 112, 114, 101, 99, 101, 100, 101, 100, 5, 55, 24, 111, 112, 101, 110, 105, 110, 103, 126, 112, 0, 0, 46];

      let mut snappier = Snappier::new(false);

      let decoded = snappier.decode(to_decode)?;

      assert_eq!(decoded, inp);

      Ok(())
  }

  #[test]
  fn test_complete_flow() -> Result<(), Box<dyn Error>> {
    let inp = "The Rust Reference defines a raw string literal as starting with the character U+0072 (r), followed by zero or more of the character U+0023 (#) and a U+0022 (double-quote) character. The raw string body can contain any sequence of Unicode characters and is terminated only by another U+0022 (double-quote) character, followed by the same number of U+0023 (#) characters that preceded the opening U+0022 (double-quote) character.";

    let mut snappier = Snappier::new(false);

    let s = snappier
        .encode(inp, vec![])?;

    let s = snappier.decode(s)?;

    assert_eq!(s, inp);
    Ok(())
  }
}