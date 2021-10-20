import PlayerBattle from "./PlayerBattle";
export default class Battle {
    round;
    p1;
    card1;
    pillz1;
    fury1;
    p2;
    card2;
    pillz2;
    fury2;
    events1;
    events2;
    b1;
    b2;
    constructor(round, p1, card1, pillz1, fury1, p2, card2, pillz2, fury2, events1, events2) {
        this.round = round;
        this.p1 = p1;
        this.card1 = card1;
        this.pillz1 = pillz1;
        this.fury1 = fury1;
        this.p2 = p2;
        this.card2 = card2;
        this.pillz2 = pillz2;
        this.fury2 = fury2;
        this.events1 = events1;
        this.events2 = events2;
        this.b1 = new PlayerBattle(round.r1, p1, card1, p2, card2, events1);
        this.b2 = new PlayerBattle(round.r2, p2, card2, p1, card1, events2);
        this.play();
    }
    play() {
        this.p1.wonPrevious = this.p1.won;
        this.p2.wonPrevious = this.p2.won;
        this.events1.executePre(this.b1);
        this.events2.executePre(this.b2);
        if (this.fury1) {
            this.card1.damage.final += 2;
        }
        if (this.fury2) {
            this.card2.damage.final += 2;
        }
        this.card1.attack.final = this.card1.power.final * (this.pillz1 + 1);
        this.card2.attack.final = this.card2.power.final * (this.pillz2 + 1);
        this.events1.executePost(this.b1);
        this.events2.executePost(this.b2);
        console.log(`\t\t\t\t\t${this.p1.name}: Attack ${this.card1.attack.final}   |   ${this.p2.name}: Attack ${this.card2.attack.final}`
            .white);
        this.p1.pillz -= this.pillz1 + (this.fury1 ? 3 : 0);
        this.p2.pillz -= this.pillz2 + (this.fury2 ? 3 : 0);
        let c1 = this.card1;
        let c2 = this.card2;
        let a1 = c1.attack.final;
        let a2 = c2.attack.final;
        if (a1 > a2 ||
            (a1 == a2 && c1.stars < c2.stars) ||
            (c1.stars == c2.stars && this.round.first && a1 == a2)) {
            this.p1.won = this.card1.won = true;
            this.p2.won = this.card2.won = false;
            this.p2.life -= this.card1.damage.final;
        }
        else {
            this.p1.won = this.card1.won = false;
            this.p2.won = this.card2.won = true;
            this.p1.life -= this.card2.damage.final;
        }
        this.events1.executeEnd(this.b1);
        this.events2.executeEnd(this.b2);
        this.card1.played = true;
        this.card2.played = true;
    }
}
//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiQmF0dGxlLmpzIiwic291cmNlUm9vdCI6Ii9DOi9Vc2Vycy9TdHVkZW50L0RvY3VtZW50cy9Ob2RlSlNXb3Jrc3BhY2UvVXJiYW5SZWNyZWF0aW9uL3NyYy8iLCJzb3VyY2VzIjpbInRzL0JhdHRsZS50cyJdLCJuYW1lcyI6W10sIm1hcHBpbmdzIjoiQUFHQSxPQUFPLFlBQVksTUFBTSxnQkFBZ0IsQ0FBQztBQUkxQyxNQUFNLENBQUMsT0FBTyxPQUFPLE1BQU07SUFDekIsS0FBSyxDQUFRO0lBQ2IsRUFBRSxDQUFTO0lBQ1gsS0FBSyxDQUFPO0lBQ1osTUFBTSxDQUFTO0lBQ2YsS0FBSyxDQUFVO0lBQ2YsRUFBRSxDQUFTO0lBQ1gsS0FBSyxDQUFPO0lBQ1osTUFBTSxDQUFTO0lBQ2YsS0FBSyxDQUFVO0lBQ2YsT0FBTyxDQUFTO0lBQ2hCLE9BQU8sQ0FBUztJQUNoQixFQUFFLENBQWU7SUFDakIsRUFBRSxDQUFlO0lBQ2pCLFlBQ0UsS0FBWSxFQUNaLEVBQVUsRUFDVixLQUFXLEVBQ1gsTUFBYyxFQUNkLEtBQWMsRUFDZCxFQUFVLEVBQ1YsS0FBVyxFQUNYLE1BQWMsRUFDZCxLQUFjLEVBQ2QsT0FBZSxFQUNmLE9BQWU7UUFFZixJQUFJLENBQUMsS0FBSyxHQUFHLEtBQUssQ0FBQztRQUVuQixJQUFJLENBQUMsRUFBRSxHQUFHLEVBQUUsQ0FBQztRQUNiLElBQUksQ0FBQyxLQUFLLEdBQUcsS0FBSyxDQUFDO1FBQ25CLElBQUksQ0FBQyxNQUFNLEdBQUcsTUFBTSxDQUFDO1FBQ3JCLElBQUksQ0FBQyxLQUFLLEdBQUcsS0FBSyxDQUFDO1FBRW5CLElBQUksQ0FBQyxFQUFFLEdBQUcsRUFBRSxDQUFDO1FBQ2IsSUFBSSxDQUFDLEtBQUssR0FBRyxLQUFLLENBQUM7UUFDbkIsSUFBSSxDQUFDLE1BQU0sR0FBRyxNQUFNLENBQUM7UUFDckIsSUFBSSxDQUFDLEtBQUssR0FBRyxLQUFLLENBQUM7UUFFbkIsSUFBSSxDQUFDLE9BQU8sR0FBRyxPQUFPLENBQUM7UUFD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