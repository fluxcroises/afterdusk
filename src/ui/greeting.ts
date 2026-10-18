/** « Bonjour » le jour, « Bonsoir » à partir de 18 h. */
export function greetingKey(date = new Date()): "home.morning" | "home.evening" {
  return date.getHours() >= 18 || date.getHours() < 5 ? "home.evening" : "home.morning";
}
