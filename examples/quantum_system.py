####
# 
# source: https://www.stephendiehl.com/posts/classical_quantum/
#
####

import math
import random
import cmath
from typing import List, Tuple, Sequence

SQRT2 = math.sqrt(2.0)

class QuantumSystem:
    def __init__(self, num_qubits: int):
        if num_qubits < 1:
            raise ValueError("Number of qubits must be at least 1.")

        self.num_qubits: int = num_qubits
        self.num_states: int = 1 << num_qubits

        self.amplitudes: List[complex] = [complex(0.0)] * self.num_states
        self.amplitudes[0] = complex(1.0)

    def _validate_qubit_index(self, qubit_index: int):
        if not (0 <= qubit_index < self.num_qubits):
            raise ValueError(
                f"Invalid qubit index: {qubit_index}. "
                f"Must be between 0 and {self.num_qubits - 1}."
            )

    def _validate_qubit_indices(self, *indices: int):
        seen = set()
        for index in indices:
            self._validate_qubit_index(index)
            if index in seen:
                 raise ValueError(f"Duplicate qubit index specified: {index}")
            seen.add(index)

    def measure(self) -> Tuple[int, ...]:
        probabilities: List[float] = [abs(amp) ** 2 for amp in self.amplitudes]

        measured_state_index: int = random.choices(
            population=range(self.num_states), weights=probabilities, k=1
        )[0]

        self.amplitudes = [complex(0.0)] * self.num_states
        self.amplitudes[measured_state_index] = complex(1.0)

        measured_bits = tuple(
            (measured_state_index >> bit_pos) & 1
            for bit_pos in range(self.num_qubits)
        )
        return measured_bits

    def get_probabilities(self) -> List[float]:
        return [abs(amp) ** 2 for amp in self.amplitudes]

    def apply_h(self, target_qubit: int):
        self._validate_qubit_index(target_qubit)
        
        new_amplitudes = self.amplitudes[:] 

        target_mask = 1 << target_qubit
        
        for i in range(self.num_states // 2):
            lower_mask = target_mask - 1
            upper_mask = ~lower_mask
            i0 = (i & lower_mask) | ((i << 1) & upper_mask)
            i1 = i0 | target_mask 

            amp0 = self.amplitudes[i0]
            amp1 = self.amplitudes[i1]

            new_amp0 = (amp0 + amp1) / SQRT2
            new_amp1 = (amp0 - amp1) / SQRT2

            new_amplitudes[i0] = new_amp0
            new_amplitudes[i1] = new_amp1
            
        self.amplitudes = new_amplitudes

    def apply_cnot(self, control_qubit: int, target_qubit: int):
        self._validate_qubit_indices(control_qubit, target_qubit)

        control_mask = 1 << control_qubit
        target_mask = 1 << target_qubit
        
        new_amplitudes = self.amplitudes[:] 
            
        for i0 in range(self.num_states):
            if (i0 >> control_qubit) & 1:
                i1 = i0 ^ target_mask 
                if i0 < i1:
                    new_amplitudes[i0], new_amplitudes[i1] = \
                        self.amplitudes[i1], self.amplitudes[i0] 

        self.amplitudes = new_amplitudes

    def apply_t(self, target_qubit: int):
        self._validate_qubit_index(target_qubit)

        phase_shift = cmath.exp(1j * cmath.pi / 4.0)
        target_mask = 1 << target_qubit

        for i in range(self.num_states):
            if (i >> target_qubit) & 1:
                self.amplitudes[i] *= phase_shift
                
    def apply_original_pi_over_eight(self, target_qubit: int):
        self._validate_qubit_index(target_qubit)

        phase_zero = cmath.exp(-1j * cmath.pi / 8.0)
        phase_one = cmath.exp(1j * cmath.pi / 8.0)
        target_mask = 1 << target_qubit

        for i in range(self.num_states):
            if (i >> target_qubit) & 1: 
                self.amplitudes[i] *= phase_one
            else: 
                self.amplitudes[i] *= phase_zero

    def __repr__(self) -> str:
        state_strs = []
        for i, amp in enumerate(self.amplitudes):
            if not cmath.isclose(amp, 0.0):
                basis_state = format(i, f'0{self.num_qubits}b')
                state_strs.append(f"{amp:.3f}|{basis_state}>")
        if not state_strs:
            return "QuantumSystem(num_qubits={}, state=Zero Vector)".format(
                self.num_qubits
            )
        return " + ".join(state_strs)

    def __str__(self) -> str:
        return self.__repr__()
