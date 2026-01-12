import type { PassageText } from '../types';

interface Props {
  title: string;
  bookTitle: string;
  location: string;
  text: PassageText;
}

export function PassageDisplay({ title, bookTitle, location, text }: Props) {
  return (
    <div className="p-4 border rounded-lg bg-white">
      <div className="mb-3">
        <h3 className="font-bold text-lg">{title}</h3>
        <p className="text-sm text-gray-600">{bookTitle}</p>
        <p className="text-sm text-gray-500">{location}</p>
      </div>
      <div className="arabic-text text-right leading-loose" dir="rtl" lang="ar">
        <span className="context-text">{text.before}</span>
        {text.before && ' '}
        <span className="highlight-match">{text.matched}</span>
        {text.after && ' '}
        <span className="context-text">{text.after}</span>
      </div>
    </div>
  );
}
