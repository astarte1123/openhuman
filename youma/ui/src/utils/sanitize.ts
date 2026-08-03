// Strip markdown images, emoji, and horizontal rules from AI response text.
const EMOJI_RE = /[\u{1F000}-\u{1FFFF}\u{2600}-\u{27BF}\u{2300}-\u{23FF}\u{2B00}-\u{2BFF}\u{FE00}-\u{FEFF}\u{1F900}-\u{1F9FF}]/gu;
const MD_IMAGE_RE = /!\[[^\]]*\]\([^)]*\)/g;
// Matches ---, ***, ___, and any repetition of -, *, _ (3+) on their own line
const MD_HR_RE = /^[ \t]*([*\-_][ \t]*){3,}[ \t]*$/gm;

export function sanitizeAIContent(text: string): string {
  return text
    .replace(MD_IMAGE_RE, '')
    .replace(MD_HR_RE, '')
    .replace(EMOJI_RE, '')
    .replace(/\n{3,}/g, '\n\n');
}
