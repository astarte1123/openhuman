import createDebug from 'debug';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import HighlightedSnippet from '../components/meetings/HighlightedSnippet';
import MeetingListRow from '../components/meetings/MeetingListRow';
import MeetingRecordModal from '../components/meetings/MeetingRecordModal';
import { useT } from '../lib/i18n/I18nContext';
import {
  type Meeting,
  auraMeetingsList,
  auraMeetingsSearch,
  type TranscriptSearchResult,
} from '../utils/tauriCommands/meetings';

const log = createDebug('app:pages:Meetings');

// Mirrors AUTOCOMPLETE_POLL_DEBOUNCE_MS's window.setTimeout/clearTimeout
// pattern in useInlineAutocomplete.ts.
const SEARCH_DEBOUNCE_MS = 300;

const Meetings = () => {
  const { t } = useT();
  const navigate = useNavigate();

  const [meetings, setMeetings] = useState<Meeting[]>([]);
  const [loading, setLoading] = useState(true);
  const [coreError, setCoreError] = useState<string | null>(null);
  const [showRecordModal, setShowRecordModal] = useState(false);

  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<TranscriptSearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const searchDebounceRef = useRef<number | null>(null);
  const searchRequestSeqRef = useRef(0);

  const loadMeetings = useCallback(async () => {
    log('loadMeetings: start');
    setLoading(true);
    setCoreError(null);
    try {
      const response = await auraMeetingsList();
      setMeetings(response.result);
      log('loadMeetings: success count=%d', response.result.length);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      log('loadMeetings: failure %s', message);
      setCoreError(message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadMeetings();
  }, [loadMeetings]);

  const trimmedQuery = searchQuery.trim();

  useEffect(() => {
    if (searchDebounceRef.current !== null) {
      window.clearTimeout(searchDebounceRef.current);
      searchDebounceRef.current = null;
    }

    if (trimmedQuery.length === 0) {
      // Invalidate any in-flight/pending search so a slow response can't
      // land after the box is cleared. No need to reset searchResults /
      // searchError / searching here: the render below only reads them
      // while `isSearchActive` is true, which is never the case when the
      // trimmed query is empty, so stale values are simply never shown.
      searchRequestSeqRef.current += 1;
      return;
    }

    searchDebounceRef.current = window.setTimeout(() => {
      const requestSeq = searchRequestSeqRef.current + 1;
      searchRequestSeqRef.current = requestSeq;
      setSearching(true);
      setSearchError(null);

      void auraMeetingsSearch(trimmedQuery)
        .then(response => {
          if (searchRequestSeqRef.current !== requestSeq) return;
          setSearchResults(response.result);
          log('search: success query=%s count=%d', trimmedQuery, response.result.length);
        })
        .catch(err => {
          if (searchRequestSeqRef.current !== requestSeq) return;
          const message = err instanceof Error ? err.message : String(err);
          log('search: failure query=%s %s', trimmedQuery, message);
          setSearchError(message);
          setSearchResults([]);
        })
        .finally(() => {
          if (searchRequestSeqRef.current !== requestSeq) return;
          setSearching(false);
        });
    }, SEARCH_DEBOUNCE_MS);

    return () => {
      if (searchDebounceRef.current !== null) {
        window.clearTimeout(searchDebounceRef.current);
        searchDebounceRef.current = null;
      }
    };
  }, [trimmedQuery]);

  const isSearchActive = trimmedQuery.length > 0;

  return (
    <div className="space-y-4 p-4 pt-6">
      <div className="mx-auto max-w-2xl overflow-hidden rounded-2xl border border-stone-200 bg-white shadow-soft">
        <div className="flex items-center justify-between border-b border-stone-100 px-4 py-3">
          <div>
            <h1 className="text-lg font-semibold text-stone-900">{t('meetings.title')}</h1>
            <p className="text-xs text-stone-500">{t('meetings.subtitle')}</p>
          </div>
          <button
            type="button"
            onClick={() => setShowRecordModal(true)}
            className="rounded-lg bg-primary-500 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-primary-600">
            {t('meetings.newRecording')}
          </button>
        </div>

        {coreError && (
          <div className="mx-4 mt-3 rounded-lg border border-coral-200 bg-coral-50 px-3 py-2 text-xs text-coral-700">
            {coreError}
          </div>
        )}

        <div className="border-b border-stone-100 px-4 py-3">
          <input
            type="text"
            value={searchQuery}
            onChange={e => setSearchQuery(e.target.value)}
            placeholder={t('meetings.searchPlaceholder')}
            className="w-full rounded-lg border border-stone-200 px-3 py-1.5 text-sm text-stone-900 placeholder:text-stone-400 focus:border-primary-300 focus:outline-none"
          />
        </div>

        {isSearchActive ? (
          <>
            {searchError && (
              <div className="mx-4 mt-3 rounded-lg border border-coral-200 bg-coral-50 px-3 py-2 text-xs text-coral-700">
                {searchError}
              </div>
            )}
            {searching ? (
              <div className="px-6 py-16 text-center text-sm text-stone-500">
                {t('common.loading')}
              </div>
            ) : searchResults.length === 0 ? (
              <div className="px-6 py-16 text-center text-sm text-stone-500">
                {t('meetings.searchNoResults')}
              </div>
            ) : (
              <ul className="divide-y divide-stone-100">
                {searchResults.map(result => (
                  <li key={result.chunk_id}>
                    <button
                      type="button"
                      onClick={() => navigate(`/meetings/${result.meeting_id}`)}
                      className="flex w-full flex-col items-start gap-1 px-4 py-3 text-left transition-colors hover:bg-stone-50">
                      <p className="truncate text-sm font-semibold text-stone-900">
                        {result.meeting_title}
                      </p>
                      <p className="text-xs text-stone-600">
                        <HighlightedSnippet snippet={result.snippet} />
                      </p>
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </>
        ) : loading ? (
          <div className="px-6 py-16 text-center text-sm text-stone-500">{t('common.loading')}</div>
        ) : meetings.length === 0 ? (
          <div className="px-6 py-16 text-center text-sm text-stone-500">{t('meetings.empty')}</div>
        ) : (
          <ul className="divide-y divide-stone-100">
            {meetings.map(meeting => (
              <MeetingListRow key={meeting.id} meeting={meeting} />
            ))}
          </ul>
        )}
      </div>

      {showRecordModal && (
        <MeetingRecordModal
          onClose={() => setShowRecordModal(false)}
          onStarted={meetingId => {
            log('recording started meeting_id=%s', meetingId);
            setShowRecordModal(false);
            navigate(`/meetings/${meetingId}`);
          }}
        />
      )}
    </div>
  );
};

export default Meetings;
