import { EventTime } from "../Events";
export default class Modifier {
    constructor() {
        this.eventTime = EventTime.START;
        this.win = undefined;
    }
    static from(o) {
        return Object.setPrototypeOf(o, Modifier.prototype);
    }
}
//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiTW9kaWZpZXIuanMiLCJzb3VyY2VSb290IjoiL0M6L1VzZXJzL1N0dWRlbnQvRG9jdW1lbnRzL05vZGVKU1dvcmtzcGFjZS9VcmJhblJlY3JlYXRpb24vc3JjLyIsInNvdXJjZXMiOlsidHMvbW9kaWZpZXJzL01vZGlmaWVyLnRzIl0sIm5hbWVzIjpbXSwibWFwcGluZ3MiOiJBQUNBLE9BQU8sRUFBRSxTQUFTLEVBQUUsTUFBTSxXQUFXLENBQUM7QUFHdEMsTUFBTSxDQUFDLE9BQU8sT0FBZ0IsUUFBUTtJQUF0QztRQUNFLGNBQVMsR0FBRyxTQUFTLENBQUMsS0FBSyxDQUFDO1FBQzVCLFFBQUcsR0FBYSxTQUFTLENBQUM7SUFLNUIsQ0FBQztJQUpDLE1BQU0sQ0FBQyxJQUFJLENBQUMsQ0FBVztRQUNyQixPQUFPLE1BQU0sQ0FBQyxjQUFjLENBQUMsQ0FBQyxFQUFFLFFBQVEsQ0FBQyxTQUFTLENBQUMsQ0FBQTtJQUNyRCxDQUFDO0NBRUYifQ==