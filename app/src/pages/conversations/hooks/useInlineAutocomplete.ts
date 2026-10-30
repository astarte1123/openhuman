import { useEffect, useRef, useState } from 'react';

import { auraAutocompleteCurrent, isTauri } from '../../../utils/tauriCommands';
import type { InputMode } from '../../Conversations';

const AUTOCOMPLETE_POLL_DEBOUNCE_MS = 320;
const AUTOCOMPLETE_MIN_CONTEXT_CHARS = 3;

export const useInlineAutocomplete = ({
  activeThreadId,
  inputValue,
  inputMode,
  rustChat,
}: {
  activeThreadId: string | null;
  inputValue: string;
  inputMode: InputMode;
  rustChat: boolean;
}) => {
  const [inlineSuggestionValue, setInlineSuggestionValue] = useState('');
  const autocompleteDebounceRef = useRef<number | null>(null);
  const autocompleteRequestSeqRef = useRef(0);

  useEffect(() => {
    if (
      !isTauri() ||
      !rustChat ||
      inputMode !== 'text' ||
      Boolean(activeThreadId) ||
      inputValue.trim().length < AUTOCOMPLETE_MIN_CONTEXT_CHARS
    ) {
      setInlineSuggestionValue('');
      return;
    }

    if (autocompleteDebounceRef.current !== null) {
      window.clearTimeout(autocompleteDebounceRef.current);
    }

    autocompleteDebounceRef.current = window.setTimeout(() => {
      const requestSeq = autocompleteRequestSeqRef.current + 1;
      autocompleteRequestSeqRef.current = requestSeq;

      void auraAutocompleteCurrent({ context: inputValue })
        .then(response => {
          if (autocompleteRequestSeqRef.current !== requestSeq) return;
          setInlineSuggestionValue(response.result.suggestion?.value ?? '');
        })
        .catch(() => {
          if (autocompleteRequestSeqRef.current !== requestSeq) return;
          setInlineSuggestionValue('');
        });
    }, AUTOCOMPLETE_POLL_DEBOUNCE_MS);

    return () => {
      if (autocompleteDebounceRef.current !== null) {
        window.clearTimeout(autocompleteDebounceRef.current);
        autocompleteDebounceRef.current = null;
      }
    };
  }, [activeThreadId, inputValue, inputMode, rustChat]);

  return { inlineSuggestionValue, setInlineSuggestionValue };
};
