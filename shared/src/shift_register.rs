#[derive(PartialEq, Default)]
pub struct ShiftRegister {
    data: [Option<u64>; 3],
    write_index: usize,
}

impl ShiftRegister {
    pub fn insert(&mut self, val: u64) {
        self.data[self.write_index] = Some(val);
        self.write_index += 1;
        self.write_index %= 3;
    }

    pub fn avg(&self) -> u64 {
        let mut sum = 0u64;
        let mut count = 0u64;
        for &val in &self.data {
            if let Some(num) = val {
                sum += num;
                count += 1;
            }
        }
        if count == 0 {
            0
        } else {
            sum / count
        }
    }

    /// Don't use this. It's for testing :-)
    fn as_array(&self) -> &[Option<u64>] {
        &self.data
    }
}

#[test]
fn insert_works() {
    let mut sr = ShiftRegister::default();

    sr.insert(1);
    assert_eq!(sr.as_array(), &[Some(1), None, None]);

    sr.insert(2);
    assert_eq!(sr.as_array(), &[Some(2), Some(1), None]);

    sr.insert(3);
    assert_eq!(sr.as_array(), &[Some(3), Some(2), Some(1)]);

    sr.insert(4);
    assert_eq!(sr.as_array(), &[Some(4), Some(3), Some(2)]);
}

#[test]
fn avg_works() {
    let mut sr = ShiftRegister::default();

    sr.insert(1);
    sr.insert(2);
    sr.insert(3);

    assert_eq!(sr.avg(), 1);

    sr.insert(4);

    assert_eq!(sr.avg(), 1);
}
