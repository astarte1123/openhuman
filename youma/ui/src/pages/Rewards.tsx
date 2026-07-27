import { useEffect, useState } from 'react';
import { getRewards } from '../api';
import type { RewardsInfo } from '../api';

const TIERS = [
  { name: 'Explorer', min: 0,    emoji: '🌍' },
  { name: 'Builder',  min: 100,  emoji: '🔨' },
  { name: 'Pioneer',  min: 500,  emoji: '🚀' },
  { name: 'Legend',   min: 1000, emoji: '⭐' },
];

export default function RewardsPage() {
  const [info, setInfo] = useState<RewardsInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    getRewards().then(setInfo).finally(() => setLoading(false));
  }, []);

  async function handleCopy() {
    if (!info) return;
    await navigator.clipboard.writeText(info.referral_code).catch(() => {});
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  const pts = info?.points ?? 0;
  const currentTier = [...TIERS].reverse().find(t => pts >= t.min) ?? TIERS[0];
  const nextTier = TIERS[TIERS.indexOf(currentTier) + 1];
  const progress = nextTier
    ? Math.min(100, ((pts - currentTier.min) / (nextTier.min - currentTier.min)) * 100)
    : 100;

  return (
    <div className="flex flex-col" style={{ height: 'calc(100vh - 72px)' }}>
      <div className="px-6 py-4 border-b border-stone-200 bg-white">
        <h1 className="text-lg font-semibold text-stone-900">Rewards</h1>
        <p className="text-xs text-stone-500 mt-0.5">Referrals and community recognition</p>
      </div>

      <div className="flex-1 overflow-y-auto bg-stone-50 px-4 py-4 space-y-4">
        {loading ? (
          <div className="space-y-3">{[1,2,3].map(i => <div key={i} className="h-24 bg-stone-200 rounded-xl animate-pulse" />)}</div>
        ) : info ? (
          <>
            <div className="bg-white rounded-xl border border-stone-200 p-5 shadow-sm">
              <div className="flex items-center justify-between mb-3">
                <div>
                  <p className="text-xs text-stone-500 uppercase tracking-wide font-medium">Current Tier</p>
                  <p className="text-2xl font-bold text-stone-900 mt-0.5">{currentTier.name}</p>
                </div>
                <div className="w-14 h-14 rounded-full bg-primary-50 border-2 border-primary-200 flex items-center justify-center text-2xl">
                  {currentTier.emoji}
                </div>
              </div>
              <div className="flex items-center gap-2 mb-1">
                <div className="flex-1 h-2 bg-stone-100 rounded-full overflow-hidden">
                  <div className="h-full bg-primary-500 rounded-full transition-all duration-500" style={{ width: `${progress}%` }} />
                </div>
                <span className="text-xs text-stone-500 flex-shrink-0">{pts} pts</span>
              </div>
              {nextTier && <p className="text-xs text-stone-400">{nextTier.min - pts} points to {nextTier.name}</p>}
            </div>

            <div className="bg-white rounded-xl border border-stone-200 p-5 shadow-sm">
              <p className="text-xs text-stone-500 uppercase tracking-wide font-medium mb-3">Your Referral Code</p>
              <div className="flex items-center gap-3">
                <div className="flex-1 font-mono text-lg font-bold text-primary-600 bg-primary-50 border border-primary-100 rounded-lg px-4 py-2.5 text-center tracking-widest">
                  {info.referral_code}
                </div>
                <button
                  onClick={handleCopy}
                  className={`flex-shrink-0 px-4 py-2.5 rounded-lg text-sm font-medium transition-colors ${
                    copied ? 'bg-green-100 text-green-700 border border-green-200' : 'bg-primary-500 hover:bg-primary-600 text-white'
                  }`}
                >
                  {copied ? 'Copied!' : 'Copy'}
                </button>
              </div>
              <p className="text-xs text-stone-400 mt-2">Share this code with friends. You earn points when they sign up.</p>
            </div>

            <div className="grid grid-cols-2 gap-3">
              <div className="bg-white rounded-xl border border-stone-200 p-4 shadow-sm text-center">
                <p className="text-2xl font-bold text-stone-900">{info.referrals}</p>
                <p className="text-xs text-stone-500 mt-0.5">Referrals</p>
              </div>
              <div className="bg-white rounded-xl border border-stone-200 p-4 shadow-sm text-center">
                <p className="text-2xl font-bold text-primary-600">{pts}</p>
                <p className="text-xs text-stone-500 mt-0.5">Points</p>
              </div>
            </div>

            <div className="bg-white rounded-xl border border-stone-200 p-4 shadow-sm">
              <p className="text-xs text-stone-500 uppercase tracking-wide font-medium mb-3">All Tiers</p>
              <div className="space-y-1">
                {TIERS.map(tier => (
                  <div key={tier.name} className={`flex items-center justify-between py-2 px-3 rounded-lg ${currentTier.name === tier.name ? 'bg-primary-50 border border-primary-100' : ''}`}>
                    <div className="flex items-center gap-2">
                      <span>{tier.emoji}</span>
                      <span className="text-sm font-medium text-stone-800">{tier.name}</span>
                    </div>
                    <span className="text-xs text-stone-500">{tier.min === 0 ? 'Start' : `${tier.min}+ pts`}</span>
                  </div>
                ))}
              </div>
            </div>
          </>
        ) : null}
      </div>
    </div>
  );
}
