import EventTime from "../types/EventTime";
export default class Modifier {
    constructor() {
        this.eventTime = EventTime.START;
        this.win = undefined;
    }
    static from(o) {
        return Object.setPrototypeOf(o, Modifier.prototype);
    }
}
//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiTW9kaWZpZXIuanMiLCJzb3VyY2VSb290IjoiL0M6L1VzZXJzL1N0dWRlbnQvRG9jdW1lbnRzL05vZGVKU1dvcmtzcGFjZS9VcmJhblJlY3JlYXRpb24vc3JjLyIsInNvdXJjZXMiOlsidHMvbW9kaWZpZXJzL01vZGlmaWVyLnRzIl0sIm5hbWVzIjpbXSwibWFwcGluZ3MiOiJBQUNBLE9BQU8sU0FBUyxNQUFNLG9CQUFvQixDQUFDO0FBRzNDLE1BQU0sQ0FBQyxPQUFPLE9BQWdCLFFBQVE7SUFBdEM7UUFDRSxjQUFTLEdBQUcsU0FBUyxDQUFDLEtBQUssQ0FBQztRQUM1QixRQUFHLEdBQWEsU0FBUyxDQUFDO0lBSzVCLENBQUM7SUFKQyxNQUFNLENBQUMsSUFBSSxDQUFDLENBQVc7UUFDckIsT0FBTyxNQUFNLENBQUMsY0FBYyxDQUFDLENBQUMsRUFBRSxRQUFRLENBQUMsU0FBUyxDQUFDLENBQUE7SUFDckQsQ0FBQztDQUVGIn0=