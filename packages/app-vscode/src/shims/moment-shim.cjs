/**
 * Minimal moment.js shim for file-stream-rotator.
 *
 * Only implements the subset used by winston-daily-rotate-file:
 *   moment() / moment().local() / moment().utc()
 *   .format(pattern)   — YYYY, MM, DD, HH, mm, ss
 *   .minutes() / .hour() — getter and setter
 *   .subtract(amount, unit) — subtract time
 *   .add(amount, unit) — add time
 *   .endOf(unit) — end of time unit
 *   .valueOf() — unix timestamp in ms
 *
 * This replaces the full moment.js (~176KB minified) with ~2KB.
 */
'use strict';

function formatDate(date, fmt) {
  const Y = String(date.getFullYear());
  const M = String(date.getMonth() + 1).padStart(2, '0');
  const D = String(date.getDate()).padStart(2, '0');
  const H = String(date.getHours()).padStart(2, '0');
  const m = String(date.getMinutes()).padStart(2, '0');
  const s = String(date.getSeconds()).padStart(2, '0');

  return fmt
    .replace('YYYY', Y)
    .replace('MM', M)
    .replace('DD', D)
    .replace('HH', H)
    .replace('mm', m)
    .replace('ss', s);
}

/**
 * 将 amount + unit 转换为毫秒数
 */
function toMs(amount, unit) {
  const n = Number(amount) || 0;
  switch (String(unit).replace(/s$/, '')) { // 去掉复数 's'
    case 'year':        return n * 365 * 24 * 60 * 60 * 1000;
    case 'month':       return n * 30  * 24 * 60 * 60 * 1000;
    case 'week':        return n * 7   * 24 * 60 * 60 * 1000;
    case 'day':  case 'd': return n * 24 * 60 * 60 * 1000;
    case 'hour': case 'h': return n * 60 * 60 * 1000;
    case 'minute': case 'm': return n * 60 * 1000;
    case 'second': case 's': return n * 1000;
    default:            return 0;
  }
}

function momentShim(date) {
  const d = date instanceof Date ? new Date(date.getTime()) : new Date();
  const self = {
    local: () => self,
    utc: () => self,
    format: (fmt) => formatDate(d, fmt),
    minutes: (v) => { if (v !== undefined) { d.setMinutes(v); } return d.getMinutes(); },
    hour: (v) => { if (v !== undefined) { d.setHours(v); } return d.getHours(); },
    valueOf: () => d.getTime(),
    subtract: (amount, unit) => {
      d.setTime(d.getTime() - toMs(amount, unit));
      return self;
    },
    add: (amount, unit) => {
      d.setTime(d.getTime() + toMs(amount, unit));
      return self;
    },
    endOf: (unit) => {
      switch (String(unit)) {
        case 'day':
          d.setHours(23, 59, 59, 999);
          break;
        case 'hour':
          d.setMinutes(59, 59, 999);
          break;
        case 'minute':
          d.setSeconds(59, 999);
          break;
        case 'month':
          d.setDate(new Date(d.getFullYear(), d.getMonth() + 1, 0).getDate());
          d.setHours(23, 59, 59, 999);
          break;
        case 'year':
          d.setMonth(11, 31);
          d.setHours(23, 59, 59, 999);
          break;
      }
      return self;
    },
  };
  return self;
}

momentShim.utc = (date) => momentShim(date);

module.exports = momentShim;
