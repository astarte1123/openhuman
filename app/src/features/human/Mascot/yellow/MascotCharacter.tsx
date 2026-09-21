import { zColor } from '@remotion/zod-types';
import React from 'react';
import { AbsoluteFill } from 'remotion';
import { z } from 'zod';

import youmaMascotSrc from '../../../../assets/mascot/youma-mascot.svg';
import { useCurrentFrame } from './frameContext';
import { LoadingFace } from './LoadingFace';
import { RecordingFace } from './RecordingFace';

export const mascotSchema = z.object({
  arm: z.enum(['wave', 'none', 'steady']).default('wave'),
  face: z.enum(['normal', 'recording', 'loading']).default('normal'),
  talking: z.boolean().default(false),
  sleeping: z.boolean().default(false),
  thinking: z.boolean().default(false),
  greeting: z.boolean().default(false),
  mascotColor: z.enum(['yellow', 'burgundy', 'black', 'navy', 'green']).default('yellow'),
  recordingColor: zColor().default('#ff3b30'),
  loadingColor: zColor().default('#ffffff'),
});

export type MascotProps = z.infer<typeof mascotSchema>;

type ThinkingTiming = {
  /** Seconds at which the idle→thinking ramp begins. Default 1.0. */
  thinkInStartSec?: number;
  /** Seconds at which the idle→thinking ramp completes. Default 2.0. */
  thinkInEndSec?: number;
  /** Seconds at which the thinking→idle ramp begins. If unset, the pose holds. */
  thinkOutStartSec?: number;
  /** Seconds at which the thinking→idle ramp completes. Required if thinkOutStartSec is set. */
  thinkOutEndSec?: number;
};

/**
 * Static Youma mascot — renders the fixed `youma-mascot.svg` artwork.
 * Pose/color/animation props are accepted for API compatibility with
 * existing callers but no longer drive any procedural drawing; only the
 * `face` overlay (recording pulse / loading ring) remains functional,
 * since those signal real app state rather than mascot styling.
 */
export const MascotCharacter: React.FC<
  MascotProps & {
    idPrefix?: string;
    groundShadowOpacity?: number;
    compactArmShading?: boolean;
  } & ThinkingTiming
> = ({ face = 'normal', recordingColor = '#ff3b30', loadingColor = '#ffffff' }) => {
  const frame = useCurrentFrame();

  return (
    <AbsoluteFill style={{ justifyContent: 'center', alignItems: 'center' }}>
      <img
        src={youmaMascotSrc}
        alt="Youma mascot"
        style={{ width: '85%', height: '85%', objectFit: 'contain' }}
      />
      {/* Recording/loading overlays signal real app state, so they stay live on top of the static art. */}
      {face === 'recording' && (
        <svg
          width="20%"
          height="20%"
          viewBox="0 0 1000 1000"
          style={{ position: 'absolute', overflow: 'visible' }}>
          <g transform="translate(495, 495) scale(0.7) translate(-520, -555)">
            <RecordingFace frame={frame} fps={30} color={recordingColor} />
          </g>
        </svg>
      )}
      {face === 'loading' && (
        <svg
          width="20%"
          height="20%"
          viewBox="0 0 1000 1000"
          style={{ position: 'absolute', overflow: 'visible' }}>
          <g transform="translate(495, 495) scale(0.7) translate(-520, -555)">
            <LoadingFace frame={frame} fps={30} color={loadingColor} />
          </g>
        </svg>
      )}
    </AbsoluteFill>
  );
};
