export default class Player {
  life: number = 0;
  pillz: number = 0;
  name: string = '';
  level: number = 0;
  won?: boolean = undefined;
  wonPrevious?: boolean = undefined;
  constructor(life: number, pillz: number, name = "Player", level = 1) {
    this.life = life;
    this.pillz = pillz;
    this.name = name;
    this.level = level;
  }

  log(r: string | number) {
    let name = ` ${this.name} `.white.bgCyan.bold;
    let round = r.toString().green;
    let life = Math.max(this.life, 0).toString().red.bold;
    let pillz = Math.max(this.pillz, 0).toString().blue.bold;
    let bar =
      "[" +
      "#".repeat(this.pillz) +
      "-".repeat(Math.max(12 - this.pillz, 0)) +
      "]";
    console.log(
      `\n\n ${name}  ${"Round".green} | ${round}/${"4".green}    ${"Life".red
      } | ${life}    ${"Pillz".blue} | ${pillz}  ${bar.blue.bold}`
    );
  }

  toString() {
    return JSON.stringify(this); // this.name;
  }
}