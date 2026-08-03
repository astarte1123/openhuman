import debugFactory from 'debug';
import { useEffect, useMemo, useState } from 'react';

import type { AppDispatch } from '../../../store';
import {
  loadAgentProfiles,
  selectAgentProfile,
  upsertAgentProfile,
} from '../../../store/agentProfileSlice';
import type { AgentProfile } from '../../../types/agentProfile';
import { formatAgentProfileAgentLabel } from '../threadHelpers';

const debug = debugFactory('conversations');

export const DEFAULT_PROFILE_DRAFT = {
  name: '',
  agentId: 'orchestrator',
  systemPromptSuffix: '',
  allowedTools: '',
};

export const useAgentProfileDraft = ({
  agentProfiles,
  dispatch,
  setSendAdvisory,
}: {
  agentProfiles: AgentProfile[];
  dispatch: AppDispatch;
  setSendAdvisory: (msg: string | null) => void;
}) => {
  const [profileDraftOpen, setProfileDraftOpen] = useState(false);
  const [profileDraft, setProfileDraft] = useState(DEFAULT_PROFILE_DRAFT);

  const agentProfileAgentOptions = useMemo(() => {
    const seen = new Set<string>();
    const options: Array<{ id: string; label: string }> = [];
    for (const profile of agentProfiles) {
      const id = profile.agentId.trim();
      if (!id || seen.has(id)) continue;
      seen.add(id);
      options.push({
        id,
        label: profile.builtIn ? profile.name : formatAgentProfileAgentLabel(id),
      });
    }
    if (profileDraft.agentId && !seen.has(profileDraft.agentId)) {
      options.push({
        id: profileDraft.agentId,
        label: formatAgentProfileAgentLabel(profileDraft.agentId),
      });
    }
    if (options.length === 0) {
      options.push({ id: 'orchestrator', label: 'Orchestrator' });
    }
    return options;
  }, [agentProfiles, profileDraft.agentId]);

  const handleSelectAgentProfile = async (profileId: string) => {
    try {
      await dispatch(selectAgentProfile(profileId)).unwrap();
    } catch (error) {
      debug('agent profile select failed: %o', error);
    }
  };

  const handleCreateAgentProfile = async () => {
    const name = profileDraft.name.trim();
    if (!name) return;
    const duplicate = agentProfiles.some(
      profile => profile.name.trim().toLowerCase() === name.toLowerCase()
    );
    if (duplicate) {
      setSendAdvisory(`Agent profile "${name}" already exists.`);
      return;
    }
    const id = `profile-${globalThis.crypto.randomUUID().slice(0, 8)}`;
    const allowedTools = profileDraft.allowedTools
      .split(',')
      .map(tool => tool.trim())
      .filter(Boolean);
    const profile: AgentProfile = {
      id,
      name,
      description: 'Custom agent profile',
      agentId: profileDraft.agentId,
      systemPromptSuffix: profileDraft.systemPromptSuffix.trim() || null,
      allowedTools: allowedTools.length > 0 ? allowedTools : null,
      builtIn: false,
    };
    try {
      await dispatch(upsertAgentProfile(profile)).unwrap();
      await dispatch(selectAgentProfile(id)).unwrap();
      setProfileDraftOpen(false);
      setProfileDraft(DEFAULT_PROFILE_DRAFT);
      setSendAdvisory(null);
    } catch (error) {
      debug('agent profile create failed: %o', error);
      setSendAdvisory('Could not create agent profile.');
    }
  };

  useEffect(() => {
    void dispatch(loadAgentProfiles())
      .unwrap()
      .catch(error => {
        debug('agent profiles load failed: %o', error);
      });
  }, [dispatch]);

  return {
    profileDraftOpen,
    setProfileDraftOpen,
    profileDraft,
    setProfileDraft,
    agentProfileAgentOptions,
    handleSelectAgentProfile,
    handleCreateAgentProfile,
  };
};
