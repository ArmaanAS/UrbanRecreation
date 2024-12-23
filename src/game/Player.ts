export default class Player {
  // Bit format: 000000 00000 0 00 00
  // life 6 pillz 5 _name 1 won 2 wonPrevious 2
  private a = 0;
  constructor(life: number, pillz: number, name: 0 | 1) {
    if (life < 0 || pillz < 0) {
      throw new RangeError(
        `life and pillz must be a non-negative integer!\n Pillz: ${pillz}, life: ${life}`,
      );
    }
    this.a = (life & 0b111111) | ((pillz & 0b11111) << 6) | (name << 11);
  }

  get name() {
    return ((this.a >> 11) & 1) ? "Urban Rival" : "Player";
  }

  get life() {
    return this.a & 0b111111;
  }
  set life(n: number) {
    if (n < 0) this.a &= ~0b111111;
    else this.a = (this.a & ~0b111111) | (n & 0b111111);
  }

  get pillz() {
    return (this.a >> 6) & 0b11111;
  }
  set pillz(n: number) {
    if (n < 0) {
      throw new RangeError("pillz must be a non-negative integer: " + n);
    } else this.a = (this.a & ~(0b11111 << 6)) | ((n & 0b11111) << 6);
  }

  get won() {
    // 0 undefined, 0b10 false, 0b11 true
    const a = (this.a >> 12) & 0b11;
    return a === 0 ? undefined : a === 3;
  }
  set won(n: boolean | undefined) {
    if (n === undefined) {
      this.a &= ~(0b11 << 12);
    } else {
      this.a = (this.a & ~(0b11 << 12)) | (((+n << 1) | 0b1) << 12);
    }
  }

  get wonPrevious() {
    // 0 undefined, 2 false, 3 true
    const a = (this.a >> 14) & 0b11;
    return a === 0 ? undefined : a === 3;
  }
  set wonPrevious(n: boolean | undefined) {
    if (n === undefined) {
      this.a &= ~(0b11 << 14);
    } else {
      this.a = (this.a & ~(0b11 << 14)) | (((+n << 1) | 0b1) << 14);
    }
  }
}
// export default class Player {
//   life = 0;
//   pillz = 0;
//   _name = 0;
//   // name = '';
//   // level: number = 0;
//   won?: boolean = undefined;
//   wonPrevious?: boolean = undefined;
//   // constructor(life: number, pillz: number, name = "Player") { //, level = 1) {
//   constructor(life: number, pillz: number, name: 0 | 1) {
//     this.life = life;
//     this.pillz = pillz;
//     // this.name = name;
//     this._name = name;
//     // this.level = level;
//   }

//   get name() {
//     return this._name ? "Player" : "Urban Rival";
//   }

//   log(r: string | number) {
//     const name = ` ${this.name} `.white.bgCyan.bold;
//     const round = r.toString().green;
//     const life = Math.max(this.life, 0).toString().red.bold;
//     const pillz = Math.max(this.pillz, 0).toString().blue.bold;
//     const bar =
//       "[" +
//       "#".repeat(this.pillz) +
//       "-".repeat(Math.max(12 - this.pillz, 0)) +
//       "]";
//     console.log(
//       `\n\n ${name}  ${"Round".green} | ${round}/${"4".green}    ${"Life".red
//       } | ${life}    ${"Pillz".blue} | ${pillz}  ${bar.blue.bold}`
//     );
//   }

//   toString() {
//     return JSON.stringify(this); // this.name;
//   }
// }
