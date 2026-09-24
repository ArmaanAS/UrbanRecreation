import Ability, { AbilityType } from "./Ability.ts";
import BattleData from "./battle/BattleData.ts";
import CachedBattleData from "./battle/CachedBattleData.ts";
import BasicModifier from "./modifiers/BasicModifier.ts";
import { type Clan, type ClanId, ClanIdMap } from "@/game/types/CardTypes.ts";
import { DEBUG } from "../utils/Debug.ts";

const VICTORY_OR_DEFEAT_ONE_PILLZ = "Victory Or Defeat : +1 Pillz";
/**
 * "Bet > N Pillz: ..." only activates when the bet is strictly greater than N,
 * "including free Pillz and excluding Fury" - the server's own wording on every
 * ability carrying `betPillzLink: "more"` (captures/abilities.json 4657, 4893, 4949).
 */
const BET_MORE_PILLZ = /^Bet > (\d+) Pillz$/i;
const PR_HIDE_ID = 1568;
const PR_HIDE_LEVEL = 3;

export enum ConditionType {
  UNDEFINED = 0,
  COURAGE = 1,
  DEFEAT = 2,
  BRAWL = 3,
  GROWTH = 4,
  CONFIDENCE = 5,
  DEGROWTH = 6,
  "VICTORY OR DEFEAT" = 7,
  EQUALIZER = 8,
  SUPPORT = 9,
  TEAM = 10,
  SYMMETRY = 11,
  REVENGE = 12,
  REPRISAL = 13,
  DAY = 14,
  NIGHT = 15,
  KILLSHOT = 16,
  BACKLASH = 17,
  ASYMMETRY = 18,
  REANIMATE = 19,
  STOP = 20,
  UNISON = 21,
  INFILTRATED = 22,
  VERSUS = 23,
  AFTER = 24,
  BET = 25,
}

export default class Condition {
  s: string;
  type: ConditionType;
  stop!: string;
  clans!: Clan[];
  /** Threshold of a `Bet > N Pillz` condition. */
  bet!: number;
  constructor(s: string) {
    this.s = s;
    if (s.endsWith("]")) {
      const clanIds = [...s.matchAll(/\d+/g)].map((m) => +m[0]) as ClanId[];
      this.clans = clanIds.map((id) => ClanIdMap[id]);
      if (DEBUG) console.log("Clans", this.clans);
      if (s.startsWith("Versus")) {
        this.type = ConditionType.VERSUS;
      } else if (s.startsWith("After")) {
        this.type = ConditionType.AFTER;
      } else {
        this.type = ConditionType.INFILTRATED;
      }
    } else {
      const betMore = BET_MORE_PILLZ.exec(s);
      if (betMore !== null) {
        this.type = ConditionType.BET;
        this.bet = +betMore[1];
      } else {
        this.type =
          ConditionType[s.toUpperCase() as keyof typeof ConditionType] ??
            ConditionType.UNDEFINED;
      }
    }
    if (DEBUG) console.log("Condition", s, this.type);
  }

  static from(o: Condition): Condition {
    return Object.setPrototypeOf(o, Condition.prototype);
  }

  met(data: BattleData) {
    switch (this.type) {
      case ConditionType.DEFEAT:
        return data.player.won === false;
      // Clint City alternates day/night every 4 hours (night 06-10, 14-18, 22-02 Paris
      // time); Game takes a `night` flag and the cards swap to their night variants.
      case ConditionType.NIGHT:
        return !data.round.day;
      case ConditionType.DAY:
        return data.round.day;
      case ConditionType.COURAGE:
        return data.round.first;
      case ConditionType.REVENGE:
        return data.player.wonPrevious === false;
      case ConditionType.CONFIDENCE:
        return data.player.wonPrevious === true;
      case ConditionType.REPRISAL:
        return !data.round.first;
      case ConditionType.KILLSHOT:
        return data.card.attack.final >=
          data.oppCard.attack.final * 2;
      case ConditionType.BACKLASH:
        return data.player.won === true;
      case ConditionType.REANIMATE:
        if (data.player.won !== false) return false;
        // Reanimate is a Defeat life gain, not a lethal-only trigger. If the incoming
        // damage was lethal it starts from zero rather than a negative life total, which
        // is how the ability can prevent the KO.
        if (data.player.life < 0) data.player.life = 0;
        return true;

      case ConditionType.STOP:
        if (this.stop == "Ability") {
          return data.card.ability.cancel;
        } else if (this.stop == "Bonus") {
          return data.card.bonus.cancel;
        } else {
          return true;
        }
      case ConditionType.SYMMETRY:
        return data.card.index === data.oppCard.index;
      case ConditionType.ASYMMETRY:
        return data.card.index !== data.oppCard.index;

      case ConditionType.UNISON:
        return data.round.hand.getClanCards(data.card) === 4;
      case ConditionType.INFILTRATED:
        return this.clans.includes(data.card.clan);
      // "After [clan:56][clan:60]": only active if the card this player played in the
      // *previous* round belonged to one of the listed clans - the server's
      // previousClanRequirement, e.g. captures/abilities.json 5585 (the Tolvack bonus),
      // "if the player of Tolvack played an Oculus or Tolvack character in the previous
      // round". Round 0 has no previous card, so it never activates there.
      case ConditionType.AFTER:
        return data.round.lastClan !== undefined &&
          this.clans.includes(data.round.lastClan);
      case ConditionType.BET:
        return data.betPillz > this.bet;
      case ConditionType.VERSUS:
        return this.clans.find((c) =>
          data.round.oppHand.map((c) => c.clan).includes(c)
        ) !== undefined;
    }

    return true;
  }

  compile(data: BattleData | CachedBattleData, ability: Ability) {
    switch (this.type) {
      case ConditionType.BACKLASH:
        for (const mod of ability.mods) {
          if (mod instanceof BasicModifier) {
            mod.setOpp(false);
          }
        }
        break;

      case ConditionType.BRAWL:
        for (const mod of ability.mods) {
          if (mod instanceof BasicModifier) {
            mod.setPer("BRAWL");
          }
        }
        break;

      case ConditionType.SUPPORT:
        for (const mod of ability.mods) {
          if (mod instanceof BasicModifier) {
            mod.setPer("SUPPORT");
          }
        }
        break;

      case ConditionType.GROWTH:
        for (const mod of ability.mods) {
          if (mod instanceof BasicModifier) {
            mod.setPer("GROWTH");
          }
        }
        break;

      case ConditionType.DEGROWTH:
        for (const mod of ability.mods) {
          if (mod instanceof BasicModifier) {
            mod.setPer("DEGROWTH");
          }
        }
        break;

      case ConditionType.EQUALIZER:
        for (const mod of ability.mods) {
          if (mod instanceof BasicModifier) {
            mod.setPer("EQUALIZER");
          }
        }
        break;

      case ConditionType.DEFEAT:
        for (const mod of ability.mods) {
          if (mod instanceof BasicModifier) {
            mod.win = false;
          }
        }
        break;

      case ConditionType["VICTORY OR DEFEAT"]:
        for (const mod of ability.mods) {
          if (mod instanceof BasicModifier) {
            mod.win = false;
            // Riots' bonus is paid after a lethal loss in multiple captures. Pr Hide's
            // printed ability does the same in 1092909 r3. Keep both exceptions locked to
            // concrete printed card state: copied text must not inherit post-KO execution.
            const exactRiotsBonus = ability.type === AbilityType.BONUS &&
              data.card.clan === "Riots" &&
              data.card.bonusString === VICTORY_OR_DEFEAT_ONE_PILLZ;
            const exactPrHideAbility = ability.type === AbilityType.ABILITY &&
              data.card.id === PR_HIDE_ID &&
              data.card.stars === PR_HIDE_LEVEL &&
              data.card.abilityString === VICTORY_OR_DEFEAT_ONE_PILLZ;
            if (
              (exactRiotsBonus || exactPrHideAbility) &&
              ability.ability === "+1 Pillz" && !mod.opp &&
              mod.type?.name === "PILLZ"
            ) {
              mod.postKoPillz = true;
            }
          }
        }
        break;

      case ConditionType.REANIMATE:
        for (const mod of ability.mods) {
          if (mod instanceof BasicModifier) {
            mod.win = false;
            mod.revive = true;
          }
        }
        break;

      case ConditionType.STOP:
        if (ability.type === AbilityType.ABILITY) {
          this.stop = "Ability";
          data.card.ability.prot = true;
        } else if (ability.type === AbilityType.BONUS) {
          this.stop = "Bonus";
          data.card.bonus.prot = true;
        }
    }
  }

  /**
   * Expand the abbreviated condition names the card data prints. Normalising the ability
   * text has already dropped every "." (and the space before a colon), so "Asymm.:",
   * "Asy. :" and "Repris.:" arrive here as "Asymm", "Asy" and "Repris". An unknown name
   * would be met unconditionally, so each one is mapped to the condition its abilityData
   * names: indexRequirement "asymmetry" for 4999 "Asymm." and 5072/5073 "Asy.",
   * positionRequirement "defender" for 5275 "Repris.". "Asym." (data.json 2615), "Rev."
   * and "Brwl." (2598, 2611) have no captured abilityData; each is the only condition its
   * letters can abbreviate. Matched on the whole name, so full names pass through.
   */
  static normalise(c: string) {
    return c
      .replace(/vic.*/gi, "Victory Or Defeat")
      .replace(/conf.*/gi, "Confidence")
      .replace(/^Asy(?:m|mm)?$/i, "Asymmetry")
      .replace(/^Repris$/i, "Reprisal")
      .replace(/^Rev$/i, "Revenge")
      .replace(/^Brwl$/i, "Brawl");
  }
}
