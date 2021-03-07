use std::convert::TryInto;

use crate::pitch::Ratio;

static U8_PRIMES: &[u8] = &[
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
    101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181, 191, 193,
    197, 199, 211, 223, 227, 229, 233, 239, 241, 251,
];

// Extracted from https://en.xen.wiki/w/Comma

// 3-limit
pub const COMMA_65: Comma = Comma::from_monzo("65-comma", &[-103, 65]);
pub const COMMA_PYTHAGOREAN: Comma = Comma::from_monzo("Pythagorean comma", &[-19, 12]);
pub const COMMA_41: Comma = Comma::from_monzo("41-comma", &[65, 41]);
pub const COMMA_94: Comma = Comma::from_monzo("94-comma", &[149, 94]);
pub const COMMA_200: Comma =
    Comma::from_monzo("200-comma, Pythagorean integer cents comma", &[317, 200]);
pub const COMMA_MERCATOR: Comma = Comma::from_monzo("Mercator's comma, 53-comma", &[-84, 53]);
pub const COMMA_PYTHGOREAN_51: Comma = Comma::from_monzo("51-Pythagorean comma", &[-970, 612]);

// 5-limit
pub const COMMA1: Comma = Comma::from_monzo("small diesis, magic comma", &[-10, -1, 5]);
pub const COMMA2: Comma = Comma::from_monzo("11-15-comma, hendecatonic comma", &[43, -11, -11]);
pub const COMMA3: Comma = Comma::from_monzo("minimal diesis, tetracot comma", &[5, -9, 4]);
pub const COMMA4: Comma = Comma::from_monzo("semaja", &[-33, -7, 19]);
pub const COMMA5: Comma = Comma::from_monzo("quanic comma", &[74, -54, 5]);
pub const COMMA6: Comma = Comma::from_monzo("roda", &[20, -17, 3]);
pub const COMMA7: Comma = Comma::from_monzo("trisedodge comma", &[19, 10, -15]);
pub const COMMA8: Comma = Comma::from_monzo("maja", &[-3, -23, 17]);
pub const COMMA9: Comma = Comma::from_monzo("satin comma", &[104, -70, 3]);
pub const COMMA10: Comma = Comma::from_monzo("misneb", &[-57, 14, 15]);
pub const COMMA_SYNTONIC: Comma =
    Comma::from_monzo("syntonic / Didymos / meantone comma", &[-4, 4, -1]);
pub const COMMA12: Comma = Comma::from_monzo("maquila comma", &[49, -6, -17]);
pub const COMMA13: Comma = Comma::from_monzo("diaschisma", &[11, -4, -2]);
pub const COMMA14: Comma = Comma::from_monzo("countermeantone comma", &[10, 23, -20]);
pub const COMMA15: Comma = Comma::from_monzo("ditonma", &[-27, -2, 13]);
pub const COMMA16: Comma = Comma::from_monzo("misty comma", &[26, -12, -3]);
pub const COMMA17: Comma = Comma::from_monzo("pental comma", &[-28, 25, -5]);
pub const COMMA18: Comma = Comma::from_monzo("undim comma", &[41, -20, -4]);
pub const COMMA19: Comma = Comma::from_monzo("graviton, gravity comma", &[-13, 17, -6]);
pub const COMMA20: Comma = Comma::from_monzo("majvam", &[40, 7, -22]);
pub const COMMA21: Comma = Comma::from_monzo("quartonic", &[3, -18, 11]);
pub const COMMA22: Comma = Comma::from_monzo("untritonic comma", &[-51, 19, 9]);
pub const COMMA23: Comma = Comma::from_monzo("medium semicomma, sensipent comma", &[2, 9, -7]);
pub const COMMA24: Comma = Comma::from_monzo("tertiosec comma", &[-89, 21, 24]);
pub const COMMA25: Comma = Comma::from_monzo("Würschmidt comma", &[17, 1, -8]);
pub const COMMA26: Comma = Comma::from_monzo("counterhanson comma", &[-20, -24, 25]);
pub const COMMA27: Comma = Comma::from_monzo("semicomma, Fokker comma", &[-21, 3, 7]);
pub const COMMA28: Comma = Comma::from_monzo("escapade comma", &[32, -7, -9]);
pub const COMMA29: Comma = Comma::from_monzo("kleisma, semicomma majeur", &[-6, -5, 6]);
pub const COMMA30: Comma = Comma::from_monzo("qintosec comma", &[47, -15, -10]);
pub const COMMA31: Comma = Comma::from_monzo("59-5-comma", &[137, 0, -59]);
pub const COMMA32: Comma = Comma::from_monzo("unidecma", &[-7, 22, -12]);
pub const COMMA33: Comma = Comma::from_monzo("mutt comma", &[-44, -3, 21]);
pub const COMMA34: Comma = Comma::from_monzo("amity comma", &[9, -13, 5]);
pub const COMMA35: Comma = Comma::from_monzo("parakleisma", &[8, 14, -13]);
pub const COMMA36: Comma = Comma::from_monzo("gammic comma", &[-29, -11, 20]);
pub const COMMA37: Comma = Comma::from_monzo("squarschmidt comma", &[61, 4, -29]);
pub const COMMA38: Comma = Comma::from_monzo("Huntian 15-cycle comma", &[168, -43, -43]);
pub const COMMA39: Comma = Comma::from_monzo("56-syntonic comma", &[-225, 224, -56]);
pub const COMMA40: Comma = Comma::from_monzo("vulture comma", &[24, -21, 4]);
pub const COMMA41: Comma = Comma::from_monzo("lafa comma", &[77, -31, -12]);

// 7-limit
pub const COMMA42: Comma = Comma::from_monzo("senga", &[1, -3, -2, 3]);
pub const COMMA43: Comma = Comma::from_monzo("23-21-comma", &[-101, 23, 0, 23]);
pub const COMMA44: Comma = Comma::from_monzo(
    "septimal / Archytas' comma, Leipziger Komma",
    &[6, -2, 0, -1],
);
pub const COMMA45: Comma = Comma::from_monzo("mandos", &[7, 5, -4, -2]);
pub const COMMA46: Comma = Comma::from_monzo("33-7/5-comma", &[-16, 0, -33, 33]);
pub const COMMA47: Comma = Comma::from_monzo("Huntian 35-cycle comma", &[277, 0, -5, 4, -54]);
pub const COMMA48: Comma = Comma::from_monzo("blackjackisma", &[-10, 7, 8, -7]);
pub const COMMA49: Comma = Comma::from_monzo("squalentine", &[-9, 3, -3, 4]);
pub const COMMA50: Comma = Comma::from_monzo("keema", &[-5, -3, 3, 1]);
pub const COMMA51: Comma = Comma::from_monzo("gariboh", &[0, -2, 5, -3]);
pub const COMMA52: Comma = Comma::from_monzo("nuwell", &[1, 5, 1, -4]);
pub const COMMA53: Comma = Comma::from_monzo("small quadruple bluish", &[6, -5, -4, 4]);
pub const COMMA54: Comma = Comma::from_monzo("tolerma", &[10, -11, 2, 1]);
pub const COMMA55: Comma =
    Comma::from_monzo("25-36/35-comma, icosipentatonic comma", &[49, 50, -25, -25]);
pub const COMMA56: Comma = Comma::from_monzo("Huntian 21-cycle comma", &[123, -28, 0, -28]);
pub const COMMA57: Comma = Comma::from_monzo("mirwomo", &[-15, 3, 2, 2]);
pub const COMMA58: Comma = Comma::from_monzo("trimyna", &[-4, 1, -5, 5]);
pub const COMMA59: Comma = Comma::from_monzo("sensamagic", &[0, -5, 1, 2]);
pub const COMMA60: Comma = Comma::from_monzo("septimal semicomma, starling comma", &[1, 2, -3, 1]);
pub const COMMA61: Comma = Comma::from_monzo("slendro 34-comma", &[-137, -34, 0, 68]);
pub const COMMA62: Comma = Comma::from_monzo("octagar", &[5, -4, 3, -2]);
pub const COMMA63: Comma = Comma::from_monzo("orwellisma, orwell comma", &[6, 3, -1, -3]);
pub const COMMA64: Comma = Comma::from_monzo("35-7/5-comma", &[17, 0, 35, -35]);
pub const COMMA65: Comma =
    Comma::from_monzo("218edo (stacked 7/4 and 10/9) comma", &[22, -32, 16, -3]);
pub const COMMA66: Comma = Comma::from_monzo("34-jubilismic comma", &[-33, 0, -68, 68]);
pub const COMMA67: Comma = Comma::from_monzo("Hunt 7-cycle comma", &[73, 0, 0, -26]);
pub const COMMA68: Comma = Comma::from_monzo("31-35-comma", &[-159, 0, 31, 31]);
pub const COMMA69: Comma = Comma::from_monzo("quince", &[-15, 0, -2, 7]);
pub const COMMA70: Comma = Comma::from_monzo("gamelisma", &[-10, 1, 0, 3]);
pub const COMMA71: Comma = Comma::from_monzo("varunisma", &[-9, 8, -4, 2]);
pub const COMMA72: Comma = Comma::from_monzo("septimal kleisma, marvel comma", &[-5, 2, 2, -1]);
pub const COMMA73: Comma = Comma::from_monzo("dimcomp", &[-1, -4, 8, -4]);
pub const COMMA74: Comma = Comma::from_monzo("cataharry", &[-4, 9, -2, -2]);
pub const COMMA75: Comma = Comma::from_monzo("mirkwai", &[0, 3, 4, -5]);
pub const COMMA76: Comma = Comma::from_monzo("stearnsma", &[1, 10, 0, -6]);
pub const COMMA77: Comma = Comma::from_monzo("hemimage", &[5, -7, -1, 3]);
pub const COMMA78: Comma = Comma::from_monzo("hemimean", &[6, 0, -5, 2]);
pub const COMMA79: Comma = Comma::from_monzo("hemifamity", &[10, -6, 1, -1]);
pub const COMMA80: Comma = Comma::from_monzo("linus comma", &[11, -10, -10, 10]);
pub const COMMA81: Comma = Comma::from_monzo("porwell", &[11, 1, -3, -2]);
pub const COMMA82: Comma = Comma::from_monzo("garischisma", &[25, -14, 0, -1]);
pub const COMMA83: Comma = Comma::from_monzo("wadisma", &[-26, -1, 1, 9]);
pub const COMMA84: Comma = Comma::from_monzo("quasiorwellisma", &[22, -1, -10, 1]);

// 11-limit
pub const COMMA85: Comma = Comma::from_monzo("thuja comma", &[15, 0, 1, 0, -5]);
pub const COMMA86: Comma = Comma::from_monzo("sensmus", &[4, -5, -1, 1, 1]);
pub const COMMA87: Comma = Comma::from_monzo("sevnothrush", &[5, 2, -5, 0, 1]);
pub const COMMA88: Comma = Comma::from_monzo("cassacot", &[-1, 0, 1, 2, -2]);
pub const COMMA89: Comma = Comma::from_monzo("mothwellsma", &[-1, 2, 0, -2, 1]);
pub const COMMA90: Comma = Comma::from_monzo("ptolemisma", &[2, -2, 2, 0, -1]);
pub const COMMA91: Comma = Comma::from_monzo("hemimin", &[6, 1, 0, 1, -3]);
pub const COMMA92: Comma = Comma::from_monzo("biyatisma", &[-3, -1, -1, 0, 2]);
pub const COMMA93: Comma = Comma::from_monzo("aphrowe", &[0 - 3, 0, -2, 3]);
pub const COMMA94: Comma = Comma::from_monzo("valinorsma", &[4, 0, -2, -1, 1]);
pub const COMMA95: Comma = Comma::from_monzo("pentacircle", &[7, -4, 0, 1, -1]);
pub const COMMA96: Comma = Comma::from_monzo("orgonisma", &[16, 0, 0, -2, -3]);
pub const COMMA97: Comma = Comma::from_monzo("quindecic comma", &[14, -15, 0, -15, 15]);
pub const COMMA98: Comma = Comma::from_monzo("rastma", &[-1, 5, 0, 0, -2]);
pub const COMMA99: Comma = Comma::from_monzo("myhemiwell", &[2, -3, -3, 1, 2]);
pub const COMMA101: Comma = Comma::from_monzo("octatonic comma", &[15, 8, 0, 0, -8]);
pub const COMMA102: Comma = Comma::from_monzo("keenanisma", &[-7, -1, 1, 1, 1]);
pub const COMMA103: Comma = Comma::from_monzo("werckisma", &[-3, 2, -1, 2, -1]);
pub const COMMA104: Comma = Comma::from_monzo("moctdel", &[-2, 0, 3, -3, 1]);

// 13-limit
pub const COMMA105: Comma = Comma::from_monzo("wilsorma", &[-6, 0, 1, 0, 0, 1]);
pub const COMMA106: Comma = Comma::from_monzo("winmeanma", &[1, 1, -1, 0, 1, -1]);
pub const COMMA107: Comma = Comma::from_monzo("negustma", &[1, 1, 0, -1, -1, 1]);
pub const COMMA108: Comma = Comma::from_monzo("superleap", &[-1, -2, -1, 1, 0, 1]);
pub const COMMA109: Comma = Comma::from_monzo("animist", &[-3, 1, 1, 1, 0, -1]);
pub const COMMA110: Comma = Comma::from_monzo("secorian", &[12, -7, 0, 1, 0, -1]);
pub const COMMA111: Comma = Comma::from_monzo("mosaic", &[-6, 5, 0, -2, 0, 1]);
pub const COMMA112: Comma = Comma::from_monzo("gassorma", &[0, -1, 2, -1, 1, -1]);
pub const COMMA113: Comma = Comma::from_monzo("grossma", &[4, 2, 0, 0, -1, -1]);
pub const COMMA114: Comma = Comma::from_monzo("buzurgisma dhanvantarisma", &[-3, -1, 0, -1, 0, 2]);
pub const COMMA115: Comma = Comma::from_monzo("catadictma", &[-8, 2, -1, 0, 1, 1]);
pub const COMMA116: Comma = Comma::from_monzo("mynucuma", &[2, -1, -1, 2, 0, -1]);
pub const COMMA117: Comma = Comma::from_monzo("huntma nelindic comma", &[7, 0, 1, -2, 0, -1]);
pub const COMMA118: Comma = Comma::from_monzo("threedie", &[0, -7, 0, 0, 0, 3]);
pub const COMMA119: Comma = Comma::from_monzo("kestrel comma", &[2, 3, 0, -1, 1, -2]);
pub const COMMA120: Comma = Comma::from_monzo("marveltwin", &[-2, -4, 2, 0, 0, 1]);
pub const COMMA121: Comma = Comma::from_monzo("Hunt 13-cycle comma", &[-37, 0, 0, 0, 0, 10]);
pub const COMMA122: Comma = Comma::from_monzo("ratwolfsma", &[-1, 3, -2, -1, 0, 1]);
pub const COMMA123: Comma = Comma::from_monzo("minthma", &[5, -3, 0, 0, 1, -1]);
pub const COMMA124: Comma = Comma::from_monzo("gentle comma", &[2, -1, 0, 1, -2, 1]);
pub const COMMA125: Comma = Comma::from_monzo("cuthbert", &[0, 0, -1, 1, 2, -2]);

// 17-limit
pub const COMMA_SORUYO: Comma = Comma::from_monzo("soruyo unison", &[-2, -1, 1, -1, 0, 0, 1]);
pub const COMMA_LUM: Comma = Comma::from_monzo("lum comma", &[-1, 0, 0, 0, -1, -1, 2]);
pub const COMMA_MEY: Comma = Comma::from_monzo("mey comma", &[-7, 0, 0, 0, 0, 3, -1]);
pub const COMMA_SEPTENDECIMAL_HUNT: Comma = Comma::from_monzo(
    "septendecimal comma, Hunt flat 2 comma",
    &[-12, 5, 0, 0, 0, 0, 1],
);
pub const COMMA_SURUYO: Comma = Comma::from_monzo("suruyo comma", &[3, 1, 1, -1, 0, 0, -1]);
pub const COMMA_SEMITONE_23: Comma =
    Comma::from_monzo("23 semitone comma", &[-94, 0, 0, 0, 0, 0, 23]);
pub const COMMA_SOTHUTUYO: Comma = Comma::from_monzo("sothuthuyo unison", &[1, 0, 1, 0, 0, -2, 1]);
pub const COMMA_SEPTENDECIMAL_SCHISMA: Comma =
    Comma::from_monzo("septendecimal schisma", &[-7, 7, 0, 0, 0, 0, -1]);
pub const COMMA_BLUME: Comma = Comma::from_monzo("Blume comma", &[-11, 0, 0, 0, 2, 0, 1]);
pub const COMMA_SEPTEMDECIMAL_KLEISMA: Comma =
    Comma::from_monzo("septendecimal kleisma", &[8, -1, -1, 0, 0, 0, -1]);
pub const COMMA_TANNISMA: Comma = Comma::from_monzo("tannisma", &[-4, 1, 0, 1, 0, 1, -1]);
pub const COMMA_SEPTENDECIMAL_INT_CENTS: Comma = Comma::from_monzo(
    "septendecimal integer cents comma",
    &[-5, -2, 0, 0, 0, 0, 2],
);
pub const COMMA_RIPPLE_17: Comma =
    Comma::from_monzo("17-ripple integer cents comma", &[327, 0, 0, 0, 0, 0, -80]);

//19-limit
pub const COMMA_NURUYO: Comma = Comma::from_monzo("nuruyo comma", &[0, 3, 1, -1, 0, 0, 0, -1]);
pub const COMMA_NOGUGU: Comma = Comma::from_monzo("nogugu 2nd", &[2, -1, -2, 0, 0, 0, 0, 1]);
pub const COMMA_NULOZO: Comma = Comma::from_monzo(
    "nulozo unison (approximation to the adjacent step of 53edo)",
    &[-2, 0, 0, 1, 1, 0, 0, -1],
);
pub const NOVEMDECIMAL: Comma =
    Comma::from_monzo("19th-partial chroma", &[5, 1, -1, 0, 0, 0, 0, -1]);
pub const COMMA_UME: Comma = Comma::from_monzo("ume comma", &[-8, 0, 0, 0, 0, 0, 3, -1]);
pub const COMMA_NOLUZO: Comma = Comma::from_monzo("noluzo 2nd", &[-2, -1, 0, 1, -1, 0, 0, 1]);
pub const COMMA_GANASSI: Comma = Comma::from_monzo("Ganassi's comma", &[-3, 2, 0, 0, 0, 0, 1, -1]);
pub const COMMA_NOSUGU: Comma = Comma::from_monzo("nosugu unison", &[-1, 2, -1, 0, 0, 0, -1, 1]);
pub const COMMA_EYE: Comma = Comma::from_monzo("eye comma", &[3, 0, 0, 0, -2, 0, 2, -1]);
pub const COMMA_HUNT: Comma = Comma::from_monzo("Hunt 19-cycle comma", &[17, 0, 0, 0, 0, 0, 0, -4]);
pub const COMMA_BINOLU: Comma = Comma::from_monzo("binulo comma", &[0, 1, 0, 0, 2, 0, 0, -2]);
pub const COMMA_YAA: Comma = Comma::from_monzo("yama comma", &[-4, 0, 0, 0, 1, -1, 0, 1]);
pub const COMMA_SPLEEN: Comma = Comma::from_monzo("spleen comma", &[1, 1, 1, 1, -1, 0, 0, -1]);
pub const COMMA_NUTHOLOGU: Comma =
    Comma::from_monzo("nuthologu comma", &[1, -1, -1, 0, 1, 1, 0, -1]);
pub const COMMA_NUSU: Comma = Comma::from_monzo("nusu comma", &[2, 4, 0, 0, 0, 0, -1, -1]);
pub const COMMA_GO: Comma = Comma::from_monzo("go comma", &[-3, -2, -1, 0, 0, 0, 0, 2]);

pub struct Comma {
    description: &'static str,
    monzo: &'static [i16],
}

impl Comma {
    ///
    pub const fn from_monzo(description: &'static str, monzo: &'static [i16]) -> Self {
        Self { description, monzo }
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    /// Returns the prime factor represantion of a [`Comma`]
    pub fn monzo(&self) -> &'static [i16] {
        self.monzo
    }

    /// Calculates the [`Ratio`] of the [`Comma`]
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::comma;
    /// assert_approx_eq!(comma::COMMA_PYTHAGOREAN.as_ratio().as_cents(), 23.460010);
    /// assert_approx_eq!(comma::COMMA_SYNTONIC.as_ratio().as_cents(), 21.506290);
    /// ```
    pub fn as_ratio(&self) -> Ratio {
        Ratio::from_float(
            self.monzo
                .iter()
                .zip(U8_PRIMES)
                .map(|(&power, &prime)| f64::from(prime).powi(i32::from(power)))
                .product(),
        )
    }

    /// Returns the numerator and denominator of the [`Comma`]
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::comma;
    /// assert_eq!(comma::COMMA_PYTHAGOREAN.as_fraction(), (531441, 524288));
    /// assert_eq!(comma::COMMA_SYNTONIC.as_fraction(), (81, 80));
    /// ```
    pub fn as_fraction(&self) -> (u64, u64) {
        let mut numer = 1;
        let mut denom = 1;
        for (&power, &prime) in self.monzo.iter().zip(U8_PRIMES) {
            if power >= 0 {
                numer *= u64::from(prime).pow(power.try_into().unwrap())
            } else {
                denom *= u64::from(prime).pow((-power).try_into().unwrap())
            }
        }
        (numer, denom)
    }
}
