#![allow(broken_intra_doc_links)]
//! # Usage:
//! ```rust
//!     use rand_key::{RandKey, ToRandKey};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut r_p = RandKey::new("10", "2", "3")?; // For now, it's empty. Use method `join` to generate the password
//!     r_p.join()?;                                 // Now `r_p` has some content, be kept in its `key` field
//!     println!("{}", r_p);                         // Print it on the screen
//!     // One possible output: 7$pA7yMCw=2DPGN
//!     // Or you can build from an existing `&str`
//!     let mut r_p = "=tE)n5f`sidR>BV".to_randkey()?; // 10 letters, 4 symbols, 1 number
//!     // You can rebuild a random password and with equivalent amount of letters, symbols and numbers. Like below
//!     r_p.join()?;
//!     println!("{}", r_p);
//!     // One possible output: qS`Xlyhpmg~"V8[
//!     // But you have to make sure that they were composed of ASCII characters or it will return `Err`.
//!     assert!("🦀️🦀️🦀️".to_randkey().is_err());
//! #   Ok(())
//! # }
//! ```
//! # The `UNIT` field
//! The UNIT field is used to help process large number in concurrent way.
//!
//! If you want to generate a huge random password with 1 million letters, symbols and numbers each,
//! our program will accept such a sequence: [1M, 1M, 1M].
//! However, it takes up huge RAM(Because these numbers are represented in `BigUint`, kind of a `Vec`).
//! And the procedure is single-threaded, you can only process them one by one.
//!
//! The approach is to divide these large numbers into many small numbers,
//! and then process these small numbers in parallel,
//! so the small numbers here can be understood as `UNIT`.
//! For 1M(1 000 000) letters, we set 1K(1000) as the unit value, so [1M] = [1K, 1K, …, 1K] (1000 ones).
//! And we just need to hand this sequence to [rayon](https://github.com/rayon-rs/rayon) for processing.
//! But the disadvantages are also obvious, if `UNIT` number is too small, like `1`,
//! Threads did nothing useful! And capcity of the `Vec` is 1M at least!
//! It will take up huge even all RAM and may harm your computer.

#![allow(non_snake_case)]
#![deny(unused, dead_code)]


mod error;
mod prelude;
mod utils;


use {utils::*, error::GenError, std::cell::RefCell, self::ASCIIExcludeCtrl::*, crate::prelude::{AsBiguint, _DEFAULT_UNIT,}};




/// struct `RandKey`
#[derive(Clone, Debug)]
pub struct RandKey {
    ltr_cnt: BigUint,
    sbl_cnt: BigUint,
    num_cnt: BigUint,
    key:     RefCell<String>,
    UNIT:    RefCell<BigUint>,
    DATA:    Vec<Vec<String>>,
}


/// A generic trait for converting a value to a `RandKey`.
pub trait ToRandKey {
    /// Converts the value of `self` to a `RandKey`.
    fn to_randkey(&self) -> Result<RandKey, GenError>;
}


/// Valid Operations for `RandKey::set_key()`
pub enum SetRandKeyOp {
    Update,
    Check,
}


pub enum ASCIIExcludeCtrl {
    Alphabetic,
    Punctuation,
    Digit,
}


impl RandKey {
    /// Return an empty instance of `Result<RandKey, impl Error>`
    /// # Example
    ///
    /// Basic usage:
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use rand_key::RandKey;
    /// let mut r_p = RandKey::new("11", "4", "2")?;
    /// #   Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn new<L, S, N>(ltr_cnt: L, sbl_cnt: S, num_cnt: N) -> Result<Self, GenError>
    where
        L: AsRef<str>,
        S: AsRef<str>,
        N: AsRef<str>,
    {
        if Self::check_init((&ltr_cnt, &sbl_cnt, &num_cnt)) {
            Ok(RandKey {
                ltr_cnt: ltr_cnt.as_biguint()?,
                sbl_cnt: sbl_cnt.as_biguint()?,
                num_cnt: num_cnt.as_biguint()?,
                key:     RefCell::new(String::new()),
                UNIT:    RefCell::new(BigUint::from(_DEFAULT_UNIT)),
                DATA:    _DEFAULT_DATA(),
            })
        } else {
            Err(GenError::InvalidNumber)
        }
    }

    #[inline]
    pub(crate) fn check_init<L, S, N>(input: (L, S, N)) -> bool
    where
        L: AsRef<str>,
        S: AsRef<str>,
        N: AsRef<str>,
    {
        input.0.as_biguint().is_ok() && input.1.as_biguint().is_ok() && input.2.as_biguint().is_ok()
    }

    /// Return the key of random password in `&str`
    /// # Example
    ///
    /// Basic usage:
    /// ```
    /// use rand_key::RandKey;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let r_p = RandKey::new("10", "2", "3")?;
    /// assert_eq!("", r_p.key());
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn key(&self) -> String { self.key.clone().into_inner() }

    /// Set the key of `RandKey`, depend on the name of operation.
    ///
    /// * **Update** : Replace the key you've passed and update the field.
    ///
    /// * **Check** : If the field of new value doesn't match the old one, it will return an `Err` or the old `key` will be replaced.
    /// # Example
    ///
    /// Basic usage:
    /// ```
    /// use rand_key::{RandKey, SetRandKeyOp::*};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Update
    /// let mut r_p = RandKey::new("10", "2", "3")?;
    ///
    /// assert!(r_p.set_key("123456", Update).is_ok());
    ///
    /// // Check
    /// let mut r_p = RandKey::new("10", "2", "3")?;
    ///
    /// assert!(r_p.set_key("]EH1zyqx3Bl/F8a", Check).is_ok());
    /// assert!(r_p.set_key("123456", Check).is_err());
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    #[rustfmt::skip]
    pub fn set_key(&mut self, val: &str, op: SetRandKeyOp) -> Result<(), GenError> {

        use self::SetRandKeyOp::*;
        let (val_ltr_cnt, val_sbl_cnt, val_num_cnt) = _CNT(val)?;

        let mut mut_ref_key = self.key.borrow_mut();

        match op {

            Update => {
                self.ltr_cnt = val_ltr_cnt;
                self.sbl_cnt = val_sbl_cnt;
                self.num_cnt = val_num_cnt;
                *mut_ref_key = val.into();

                Ok(())
            }

            Check => {
                if (&self.ltr_cnt,
                    &self.sbl_cnt,
                    &self.num_cnt,) == (&val_ltr_cnt,
                                        &val_sbl_cnt,
                                        &val_num_cnt,) {
                    *mut_ref_key = val.into();

                    Ok(())
                } else {
                    Err(GenError::InconsistentField)
                }
            }

        }

    }

    /// Return the value of `UNIT`
    /// # Example
    ///
    /// Basic Usage:
    /// ```
    /// use rand_key::RandKey;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let r_p = RandKey::new("10", "2", "3")?;
    /// // The default value of unit is 1048576
    /// assert_eq!(r_p.unit(), "1048576");
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn unit(&self) -> &str { "1048576" }

    /// [set a right `UNIT` number](https://docs.rs/rand_pwd/1.1.3/rand_pwd/#the-unit-field).
    #[inline]
    pub fn set_unit(&self, val: impl AsRef<str>) -> Result<(), GenError> {
        let val = val.as_biguint()?;
        let mut mut_ref_unit = self.UNIT.borrow_mut();

        if val == BigUint::zero() {
            Err(GenError::InvalidUnit)
        } else {
            *mut_ref_unit = val;
            Ok(())
        }
    }

    /// Return the shared reference of `DATA`
    #[inline]
    pub fn all_data(&self) -> &Vec<Vec<String>> { &self.DATA }

    /// Return data depend on given kind
    #[inline]
    pub fn data(&self, kind: ASCIIExcludeCtrl) -> &[String] {
        match kind {
            Alphabetic => &self.DATA[0],
            Punctuation => &self.DATA[1],
            Digit => &self.DATA[2],
        }
    }

    /// Clear all the data of `RandPwd`
    #[inline]
    pub fn clear_all(&mut self) { self.DATA.iter_mut().for_each(|x| x.clear()); }

    /// Clear the letters, symbols or numbers
    #[inline]
    pub fn clear(&mut self, kind: ASCIIExcludeCtrl) {
        match kind {
            Alphabetic => self.DATA[0].clear(),
            Punctuation => self.DATA[1].clear(),
            Digit => self.DATA[2].clear(),
        }
    }

    /// Check the data
    #[inline]
    #[allow(non_snake_case)]
    pub(crate) fn check_data(&self) -> Result<(), GenError> {
        let L = self.ltr_cnt.is_zero();
        let S = self.sbl_cnt.is_zero();
        let N = self.num_cnt.is_zero();

        let dl = self.DATA[0].is_empty();
        let ds = self.DATA[1].is_empty();
        let dn = self.DATA[2].is_empty();

        let dl_L = !L && dl;
        let ds_S = !S && ds;
        let dn_N = !N && dn;

        if !(dl_L || ds_S || dn_N) {
            Ok(())
        } else {
            Err(GenError::MissChar)
        }
    }

    /// Delete the data
    /// # Example
    ///
    /// Basic Usage
    /// ```
    /// use rand_key::{RandKey, ASCIIExcludeCtrl::*};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut r_p = RandKey::new("10", "2", "3")?;
    /// r_p.replace_data(&["1", "2", "a", "-"]);
    /// r_p.del_item(&["1"]);
    /// assert_eq!(r_p.data(Digit), vec!["2"]);
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn del_item(&mut self, items: &[impl AsRef<str>]) -> Result<(), GenError> {

        let mut concat_data: Vec<String> = self.DATA.concat();

        if _CHECK_ASCII(items) {

            let mut items: Vec<char> = items.iter().map(_CHAR_FROM_STR).collect();
            items.dedup_by_key(|x| *x as u8);

            if items.iter().any(|x| concat_data.contains(&x.to_string())) {
                concat_data.retain(|x| !items.contains(&_CHAR_FROM_STR(x)));
                self.DATA = _GROUP(&concat_data);

                Ok(())
            } else {
                Err(GenError::DelNonExistValue)
            }
        } else {
            Err(GenError::InvalidChar)
        }
    }

    /// Add data to the data set that `RandKey` carries
    /// # Example
    ///
    /// Basic Usage:
    /// ```
    /// use rand_key::RandKey;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut r_p = RandKey::new("10", "2", "3")?;
    /// r_p.clear_all();
    /// r_p.add_item(&["a", "0", "-"]);
    /// r_p.join().unwrap();
    /// println!("{}", r_p);
    /// // One possible output: a0-0aaaaaa0-aaa
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    #[allow(clippy::needless_range_loop)]
    pub fn add_item(&mut self, val: &[impl AsRef<str>]) -> Result<(), GenError> {
        if _CHECK_ASCII(val) {
            let val = _GROUP(val);

            for i in 0..self.DATA.len() {
                self.DATA[i].extend_from_slice(&val[i]);
                self.DATA[i].dedup_by_key(|x| _CHAR_FROM_STR(x) as u8);
            }

            Ok(())
        } else {
            Err(GenError::InvalidChar)
        }
    }

    /// Return a new `RandKey` which has the replaced data
    /// # Example
    ///
    /// Basic usage:
    /// ```
    /// use rand_key::RandKey;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut r_p = RandKey::new("10", "2", "3")?;
    /// // Missing some kinds of characters will get an Err value
    /// assert!(r_p.replace_data(&["1"]).is_err());
    /// assert!(r_p.replace_data(&["a"]).is_err());
    /// assert!(r_p.replace_data(&["-"]).is_err());
    /// assert!(r_p.replace_data(&["1", "a", "."]).is_ok());
    /// r_p.join()?;
    /// println!("{}", r_p);
    /// // One possible output: .aa1a1aaaa.a1aa
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    #[rustfmt::skip]
    pub fn replace_data(&mut self, val: &[impl AsRef<str>]) -> Result<(), GenError> {

        if _CHECK_ASCII(val) {

            self.DATA = {

                let mut ltr = vec![];
                let mut sbl = vec![];
                let mut num = vec![];

                val.iter().for_each(|x| {
                    let x = _CHAR_FROM_STR(x);

                    if x.is_ascii_alphabetic()  { ltr.push(x.into()); }
                    if x.is_ascii_punctuation() { sbl.push(x.into()); }
                    if x.is_ascii_digit()       { num.push(x.into()); }
                });

                vec![ltr, sbl, num]

            };

            self.check_data()

        } else {
            Err(GenError::InvalidChar)
        }
    }

    /// Returns the length of this `RandKey`, in both bytes and [char]s.
    /// # Example
    ///
    /// Basic usage:
    /// ```
    /// use rand_key::RandKey;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut r_p = RandKey::new("10", "2", "3")?;
    ///
    /// r_p.join()?;
    ///
    /// assert_eq!(&r_p.len(), "15");
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn len(&self) -> String { self.key.borrow().len().to_string() }

    /// Returns true if this `RandKey` has a length of zero, and false otherwise.
    #[inline]
    pub fn is_empty(&self) -> bool { self.key.borrow().is_empty() }

    /// Get count of `RandKey`
    /// # Example
    ///
    /// Basic usage:
    /// ```
    /// use rand_key::{RandKey, ASCIIExcludeCtrl::*};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let r_p = RandKey::new("10", "2", "3")?;
    ///
    /// assert_eq!(&r_p.get_cnt(Alphabetic), "10");
    /// assert_eq!(&r_p.get_cnt(Punctuation), "2");
    /// assert_eq!(&r_p.get_cnt(Digit), "3");
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn get_cnt(&self, kind: ASCIIExcludeCtrl) -> String {
        match kind {
            Alphabetic => self.ltr_cnt.to_string(),
            Punctuation => self.sbl_cnt.to_string(),
            Digit => self.num_cnt.to_string(),
        }
    }

    /// Change the count of letters, symbols or numbers of `RandKey`
    /// # Example
    ///
    /// Basic usage:
    /// ```
    /// use rand_key::{RandKey, ASCIIExcludeCtrl::*};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut r_p = RandKey::new("10", "2", "3")?;
    ///
    /// // Set the letter's count
    /// r_p.set_cnt(Alphabetic, "20");
    /// assert_eq!(&r_p.get_cnt(Alphabetic), "20");
    ///
    /// // Set the symbol's count
    /// r_p.set_cnt(Punctuation, "1000");
    /// assert_eq!(&r_p.get_cnt(Punctuation), "1000");
    ///
    /// // Set the number's count
    /// r_p.set_cnt(Digit, "0");
    /// assert_eq!(&r_p.get_cnt(Digit), "0");
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn set_cnt(&mut self, kind: ASCIIExcludeCtrl, val: impl AsRef<str>) {
        match kind {
            Alphabetic => self.ltr_cnt = val.as_biguint().unwrap(),
            Punctuation => self.sbl_cnt = val.as_biguint().unwrap(),
            Digit => self.num_cnt = val.as_biguint().unwrap(),
        }
    }

    /// Generate the password for `RandKey`
    /// # Example
    ///
    /// Basic usage:
    /// ```
    /// use rand_key::RandKey;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut r_p = RandKey::new("10", "2", "3")?;
    /// r_p.join()?;
    /// println!("{}", r_p);
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    #[rustfmt::skip]
    pub fn join(&self) -> Result<(), GenError> {

        let mut inner = self.clone();

        if inner.check_data().is_ok() {
            let unit = &inner.UNIT.clone().into_inner();
            let data = &inner.DATA;

            // TODO: - Improve readability
            let mut PWD =
                vec![(&mut inner.ltr_cnt, &data[0]),
                     (&mut inner.sbl_cnt, &data[1]),
                     (&mut inner.num_cnt, &data[2]),]
                    .into_iter()
                    .map(|(bignum, data)| {
                        _DIV_UNIT(unit, bignum)
                            .par_iter()
                            .map(|cnt| {
                                _RAND_IDX(cnt, data.len())
                                    .par_iter()
                                    .map(|idx| data[*idx].clone())
                                    .collect::<String>()
                            })
                            .collect()
                    })
                    .collect::<Vec<Vec<_>>>()
                    .concat()
                    .join("");

            // This is absolutely safe, because they are all ASCII characters except control ones.
            let bytes = unsafe { PWD.as_bytes_mut() };
            bytes.shuffle(&mut thread_rng());

            let mut mut_ref_key = self.key.borrow_mut();

            *mut_ref_key = bytes.par_iter().map(|s| *s as char).collect::<String>();

            Ok(())

        } else {
            inner.check_data()
        }
    }
}
