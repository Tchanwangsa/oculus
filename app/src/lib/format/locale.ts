/** `locale` on the Gregorian calendar and a 12-hour clock, whatever it would
 *  pick itself: a Thai region (en-TH, th-TH) defaults to the Buddhist calendar
 *  ("October 2569 BE"), and the clock is 12-hour for everyone. The locale
 *  still sets word order and the month and day names. */
export function displayTag(locale: string): string {
  try {
    return new Intl.Locale(locale, { calendar: "gregory", hourCycle: "h12" }).toString();
  } catch {
    return locale;
  }
}

function systemLocale(): string {
  return new Intl.DateTimeFormat().resolvedOptions().locale;
}

/** The locale every date and time is formatted in: the system's, through
 *  {@link displayTag}, unless a test pins one so its expected strings hold on
 *  any machine. Formats name their fields rather than using `dateStyle` or
 *  `timeStyle`, whose patterns can add an era or pad the hour. */
export let displayLocale: string = displayTag(systemLocale());

export function pinDisplayLocale(locale: string | undefined): void {
  displayLocale = displayTag(locale ?? systemLocale());
}
