import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import CustomRoutingDialog from './CustomRoutingDialog';
import type { CloudProvider, Workload } from './types';

const workload: Workload = {
  id: 'reasoning',
  group: 'chat',
  label: 'Reasoning',
  description: 'Main chat agent, meeting summarizer',
};

const openaiProvider: CloudProvider = {
  id: 'p_openai_1',
  slug: 'openai',
  label: 'OpenAI',
  endpoint: 'https://api.openai.com/v1',
  authStyle: 'bearer',
  maskedKey: 'sk-***',
};

describe('CustomRoutingDialog — model suggestions', () => {
  // The dialog previously left the model field empty when a cloud provider
  // with no prior selection was chosen, blocking Save until the user typed
  // an exact model id from memory. It should now auto-fill a known-good
  // starting point instead.

  it('auto-fills the model field with the first known suggestion when a cloud provider is selected', () => {
    const onSubmit = vi.fn();
    render(
      <CustomRoutingDialog
        workload={workload}
        initial={{ kind: 'aura' }}
        cloudProviders={[openaiProvider]}
        localModels={[]}
        ollamaRunning={false}
        onClose={() => {}}
        onSubmit={onSubmit}
      />
    );

    const modelInput = screen.getByDisplayValue('gpt-4o-mini');
    expect(modelInput).toBeInTheDocument();

    // Save should already be enabled — no need to type a model id from memory.
    expect(screen.getByRole('button', { name: /^Save$/i })).not.toBeDisabled();
  });

  it('clicking a suggestion chip fills the model field and can be saved', () => {
    const onSubmit = vi.fn();
    render(
      <CustomRoutingDialog
        workload={workload}
        initial={{ kind: 'aura' }}
        cloudProviders={[openaiProvider]}
        localModels={[]}
        ollamaRunning={false}
        onClose={() => {}}
        onSubmit={onSubmit}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'gpt-4o' }));
    expect(screen.getByDisplayValue('gpt-4o')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /^Save$/i }));
    expect(onSubmit).toHaveBeenCalledWith({
      kind: 'cloud',
      providerSlug: 'openai',
      model: 'gpt-4o',
    });
  });

  it('still disables Save when the model field is cleared', () => {
    render(
      <CustomRoutingDialog
        workload={workload}
        initial={{ kind: 'aura' }}
        cloudProviders={[openaiProvider]}
        localModels={[]}
        ollamaRunning={false}
        onClose={() => {}}
        onSubmit={vi.fn()}
      />
    );

    fireEvent.change(screen.getByDisplayValue('gpt-4o-mini'), { target: { value: '' } });
    expect(screen.getByRole('button', { name: /^Save$/i })).toBeDisabled();
  });

  it('shows no suggestions for a custom (non-built-in) provider slug', () => {
    const customProvider: CloudProvider = {
      id: 'p_custom_1',
      slug: 'my-endpoint',
      label: 'My Endpoint',
      endpoint: 'https://example.test/v1',
      authStyle: 'bearer',
      maskedKey: 'key-***',
    };

    render(
      <CustomRoutingDialog
        workload={workload}
        initial={{ kind: 'aura' }}
        cloudProviders={[customProvider]}
        localModels={[]}
        ollamaRunning={false}
        onClose={() => {}}
        onSubmit={vi.fn()}
      />
    );

    expect(screen.queryByText('Not sure? Try:')).not.toBeInTheDocument();
  });
});
